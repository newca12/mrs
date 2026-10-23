#!/usr/bin/env bash
# Repair legacy mrs TSTP proofs whose symbols or provenance names were printed
# without the quoting required by the TPTP syntax.
#
# The repair is deliberately lexical and semantics-preserving: invalid symbol
# text is quoted, not renamed. The original symbol therefore remains the same
# symbol to a proof checker.
#
# Usage:
#   sanitize_tstp.sh --input-dir proofs --output-dir repaired
#   sanitize_tstp.sh --output-dir repaired proof1.e proof2.e
#
# The input tree is never modified. Directory layout is preserved below the
# output directory.
set -euo pipefail

INPUT_DIR=""
OUTPUT_DIR=""
INPUTS=()

usage() {
    printf '%s\n' \
        "Usage: $0 --output-dir DIR [--input-dir DIR | FILE ...]" \
        "  --input-dir DIR   Recursively process .e, .p, .s, and .proof files" \
        "  --output-dir DIR  Write repaired files here; input files are unchanged" \
        "  --help             Show this help"
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --input-dir)
            [[ $# -ge 2 ]] || { usage >&2; exit 2; }
            INPUT_DIR="$2"
            shift 2
            ;;
        --output-dir)
            [[ $# -ge 2 ]] || { usage >&2; exit 2; }
            OUTPUT_DIR="$2"
            shift 2
            ;;
        --help|-h)
            usage
            exit 0
            ;;
        --*)
            printf 'Unknown option: %s\n' "$1" >&2
            usage >&2
            exit 2
            ;;
        *)
            INPUTS+=("$1")
            shift
            ;;
    esac
done

[[ -n "${OUTPUT_DIR}" ]] || { printf '%s\n' "--output-dir is required" >&2; usage >&2; exit 2; }
[[ -n "${INPUT_DIR}" || ${#INPUTS[@]} -gt 0 ]] || { printf '%s\n' "an input directory or at least one input file is required" >&2; usage >&2; exit 2; }

if [[ -n "${INPUT_DIR}" ]]; then
    [[ -d "${INPUT_DIR}" ]] || { printf 'Input directory does not exist: %s\n' "${INPUT_DIR}" >&2; exit 2; }
    if [[ ${#INPUTS[@]} -gt 0 ]]; then
        printf '%s\n' "--input-dir cannot be combined with positional input files" >&2
        exit 2
    fi
    INPUT_DIR="$(cd "${INPUT_DIR}" && pwd)"
    shopt -s nullglob globstar
    for path in "${INPUT_DIR}"/**/{*.e,*.p,*.s,*.proof}; do
        [[ -f "${path}" ]] && INPUTS+=("${path}")
    done
    shopt -u nullglob globstar
    ((${#INPUTS[@]} > 0)) || { printf 'No proof files found in %s\n' "${INPUT_DIR}" >&2; exit 1; }
fi

mkdir -p "${OUTPUT_DIR}"
OUTPUT_DIR="$(cd "${OUTPUT_DIR}" && pwd)"

if [[ -n "${INPUT_DIR}" && "${OUTPUT_DIR}" == "${INPUT_DIR}" ]]; then
    printf '%s\n' "Refusing to use the input directory as the output directory" >&2
    exit 2
fi

process_file() {
    local input="$1"
    local relative output parent

    [[ -f "${input}" ]] || { printf 'Input file does not exist: %s\n' "${input}" >&2; exit 2; }

    if [[ -n "${INPUT_DIR}" ]]; then
        relative="${input#${INPUT_DIR}/}"
    else
        relative="$(basename "${input}")"
    fi
    output="${OUTPUT_DIR}/${relative}"
    parent="$(dirname "${output}")"
    mkdir -p "${parent}"

    perl - "${input}" "${output}" <<'PERL'
use strict;
use warnings;

my ($input_path, $output_path) = @ARGV;
open my $in, '<', $input_path or die "read $input_path: $!\n";
local $/;
my $text = <$in>;
close $in or die "close $input_path: $!\n";

sub quoted_atom {
    my ($value) = @_;
    $value =~ s/\\/\\\\/g;
    $value =~ s/'/\\'/g;
    return "'$value'";
}

sub valid_lower_word {
    my ($value) = @_;
    return $value =~ /^[a-z][A-Za-z0-9_']*$/;
}

sub valid_defined_word {
    my ($value) = @_;
    return $value =~ /^\$[a-z][A-Za-z0-9_]*$/ ||
           $value =~ /^\$\$[a-z][A-Za-z0-9_]*$/;
}

sub valid_formula_name {
    my ($value) = @_;
    return 1 if $value =~ /^'.*'$/s;
    return 1 if $value =~ /^\d+$/;
    return valid_lower_word($value);
}

sub skip_single_quote {
    my ($value, $start) = @_;
    my $length = length($value);
    my $index = $start + 1;
    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq '\\') {
            $index += 2;
        } elsif ($character eq "'") {
            return $index + 1;
        } else {
            $index++;
        }
    }
    return $length;
}

sub skip_double_quote {
    my ($value, $start) = @_;
    my $length = length($value);
    my $index = $start + 1;
    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq '\\') {
            $index += 2;
        } elsif ($character eq '"') {
            return $index + 1;
        } else {
            $index++;
        }
    }
    return $length;
}

sub skip_block_comment {
    my ($value, $start) = @_;
    my $length = length($value);
    my $index = $start + 2;
    my $depth = 1;
    while ($index < $length - 1) {
        if (substr($value, $index, 2) eq '/*') {
            $depth++;
            $index += 2;
        } elsif (substr($value, $index, 2) eq '*/') {
            $depth--;
            $index += 2;
            return $index if $depth == 0;
        } else {
            $index++;
        }
    }
    return $length;
}

sub skip_line_comment {
    my ($value, $start) = @_;
    my $newline = index($value, "\n", $start + 1);
    return $newline < 0 ? length($value) : $newline + 1;
}

sub find_top_level_comma {
    my ($value, $start) = @_;
    my $length = length($value);
    my $depth = 0;
    my $index = $start;
    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
        } elsif ($character eq '"') {
            $index = skip_double_quote($value, $index);
        } elsif ($character eq '%') {
            $index = skip_line_comment($value, $index);
        } elsif ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
        } elsif ($character eq '(') {
            $depth++;
            $index++;
        } elsif ($character eq ')') {
            return undef if $depth == 0;
            $depth--;
            $index++;
        } elsif ($character eq ',' && $depth == 0) {
            return $index;
        } else {
            $index++;
        }
    }
    return undef;
}

sub find_matching_paren {
    my ($value, $open) = @_;
    my $length = length($value);
    my $depth = 0;
    my $index = $open;
    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
        } elsif ($character eq '"') {
            $index = skip_double_quote($value, $index);
        } elsif ($character eq '%') {
            $index = skip_line_comment($value, $index);
        } elsif ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
        } elsif ($character eq '(') {
            $depth++;
            $index++;
        } elsif ($character eq ')') {
            $depth--;
            return $index if $depth == 0;
            return undef if $depth < 0;
            $index++;
        } else {
            $index++;
        }
    }
    return undef;
}

sub trim_span {
    my ($value, $start, $end) = @_;
    $start++ while $start < $end && substr($value, $start, 1) =~ /\s/;
    $end-- while $end > $start && substr($value, $end - 1, 1) =~ /\s/;
    return ($start, $end);
}

sub apply_replacements {
    my ($value, $replacements) = @_;
    for my $replacement (sort { $b->[0] <=> $a->[0] } @$replacements) {
        my ($start, $end, $new_value) = @$replacement;
        substr($value, $start, $end - $start, $new_value);
    }
    return $value;
}

sub rewrite_file_sources {
    my ($value) = @_;
    my @replacements;
    my $length = length($value);
    my $index = 0;

    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
            next;
        }
        if ($character eq '"') {
            $index = skip_double_quote($value, $index);
            next;
        }
        if ($character eq '%') {
            $index = skip_line_comment($value, $index);
            next;
        }
        if ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
            next;
        }

        if (substr($value, $index, 4) eq 'file' &&
            ($index == 0 || substr($value, $index - 1, 1) !~ /[A-Za-z0-9_']/)) {
            my $after_word = $index + 4;
            $after_word++ while $after_word < $length && substr($value, $after_word, 1) =~ /\s/;
            if ($after_word < $length && substr($value, $after_word, 1) eq '(') {
                my $open = $after_word;
                my $comma = find_top_level_comma($value, $open + 1);
                my $close = find_matching_paren($value, $open);
                if (defined($comma) && defined($close) && $comma < $close) {
                    my ($start, $end) = trim_span($value, $comma + 1, $close);
                    if ($start < $end && substr($value, $start, 1) ne "'") {
                        push @replacements, [$start, $end, quoted_atom(substr($value, $start, $end - $start))];
                    }
                    $index = $close + 1;
                    next;
                }
            }
        }
        $index++;
    }
    return apply_replacements($value, \@replacements);
}

sub rewrite_formula_names {
    my ($value) = @_;
    my @replacements;
    my $length = length($value);
    my $index = 0;

    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
            next;
        }
        if ($character eq '"') {
            $index = skip_double_quote($value, $index);
            next;
        }
        if ($character eq '%') {
            $index = skip_line_comment($value, $index);
            next;
        }
        if ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
            next;
        }

        if (substr($value, $index) =~ /\A(?:fof|cnf|tff|thf|tcf|tpi)/) {
            my $word = $&;
            my $after_word = $index + length($word);
            if (($index == 0 || substr($value, $index - 1, 1) !~ /[A-Za-z0-9_\$']/)) {
                while ($after_word < $length && substr($value, $after_word, 1) =~ /\s/) {
                    $after_word++;
                }
                if ($after_word < $length && substr($value, $after_word, 1) eq '(') {
                    my $comma = find_top_level_comma($value, $after_word + 1);
                    if (defined($comma)) {
                        my ($start, $end) = trim_span($value, $after_word + 1, $comma);
                        my $name = substr($value, $start, $end - $start);
                        if ($start < $end && !valid_formula_name($name)) {
                            push @replacements, [$start, $end, quoted_atom($name)];
                        }
                    }
                }
            }
        }
        $index++;
    }
    return apply_replacements($value, \@replacements);
}

sub generated_symbol_end {
    my ($value, $start, $prefix_length) = @_;
    my $length = length($value);
    my $index = $start + $prefix_length;
    my $depth = 0;

    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq '(') {
            $depth++;
            $index++;
            next;
        }
        if ($character eq ')') {
            return undef if $depth == 0;
            $depth--;
            $index++;
            next;
        }
        if ($depth == 0 && $character eq '_' &&
            substr($value, $index + 1) =~ /\A(\d+)/) {
            my $digits = $1;
            my $end = $index + 1 + length($digits);
            my $after = $end < $length ? substr($value, $end, 1) : '';
            if ($after eq '' || $after =~ /[\s,()\[\]|&=!?~:;]/) {
                return $end;
            }
        }
        $index++;
    }
    return undef;
}

sub rewrite_generated_symbols {
    my ($value) = @_;
    my @replacements;
    my $length = length($value);
    my $index = 0;

    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
            next;
        }
        if ($character eq '"') {
            $index = skip_double_quote($value, $index);
            next;
        }
        if ($character eq '%') {
            $index = skip_line_comment($value, $index);
            next;
        }
        if ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
            next;
        }

        my ($prefix, $prefix_length);
        if (substr($value, $index, 3) eq 'sk_') {
            ($prefix, $prefix_length) = ('sk_', 3);
        } elsif (substr($value, $index, 4) eq 'def_') {
            ($prefix, $prefix_length) = ('def_', 4);
        }

        if (defined($prefix) &&
            ($index == 0 || substr($value, $index - 1, 1) !~ /[A-Za-z0-9_']/)) {
            my $end = generated_symbol_end($value, $index, $prefix_length);
            if (defined($end)) {
                my $symbol = substr($value, $index, $end - $index);
                if (!valid_lower_word($symbol)) {
                    push @replacements, [$index, $end, quoted_atom($symbol)];
                }
                $index = $end;
                next;
            }
        }
        $index++;
    }
    return apply_replacements($value, \@replacements);
}

sub rewrite_formula_numbers {
    my ($value) = @_;
    my @replacements;
    my $length = length($value);
    my $index = 0;

    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
            next;
        }
        if ($character eq '"') {
            $index = skip_double_quote($value, $index);
            next;
        }
        if ($character eq '%') {
            $index = skip_line_comment($value, $index);
            next;
        }
        if ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
            next;
        }

        if (substr($value, $index) =~ /\A(?:fof|cnf|tff|thf|tcf|tpi)/ &&
            ($index == 0 || substr($value, $index - 1, 1) !~ /[A-Za-z0-9_\$']/)) {
            my $word = $&;
            my $after_word = $index + length($word);
            $after_word++ while $after_word < $length && substr($value, $after_word, 1) =~ /\s/;
            if ($after_word < $length && substr($value, $after_word, 1) eq '(') {
                my $comma1 = find_top_level_comma($value, $after_word + 1);
                my $comma2 = defined($comma1) ? find_top_level_comma($value, $comma1 + 1) : undef;
                if (defined($comma2)) {
                    my $formula_start = $comma2 + 1;
                    my $formula_end = find_top_level_comma($value, $formula_start);
                    if (!defined($formula_end)) {
                        my $close = find_matching_paren($value, $after_word);
                        $formula_end = $close if defined($close);
                    }

                    if (defined($formula_end)) {
                        my $cursor = $formula_start;
                        while ($cursor < $formula_end) {
                            my $inner = substr($value, $cursor, 1);
                            if ($inner eq "'") {
                                $cursor = skip_single_quote($value, $cursor);
                                next;
                            }
                            if ($inner eq '"') {
                                $cursor = skip_double_quote($value, $cursor);
                                next;
                            }
                            if ($inner eq '%') {
                                $cursor = skip_line_comment($value, $cursor);
                                next;
                            }
                            if ($inner eq '/' && substr($value, $cursor, 2) eq '/*') {
                                $cursor = skip_block_comment($value, $cursor);
                                next;
                            }

                            my $number = substr($value, $cursor) =~
                                /\A[+-]?(?:\d+(?:\/\d+|\.\d+(?:[eE][+-]?\d+)?|[eE][+-]?\d+)?)/
                                ? $& : undef;
                            if (defined($number)) {
                                my $before = $cursor > $formula_start ? substr($value, $cursor - 1, 1) : '';
                                my $after = $cursor + length($number) < $formula_end
                                    ? substr($value, $cursor + length($number), 1) : '';
                                if ($before !~ /[A-Za-z0-9_']/ && $after !~ /[A-Za-z0-9_']/) {
                                    push @replacements, [
                                        $cursor,
                                        $cursor + length($number),
                                        quoted_atom($number),
                                    ];
                                    $cursor += length($number);
                                    next;
                                }
                            }
                            $cursor++;
                        }
                        $index = $formula_end;
                        next;
                    }
                }
            }
        }
        $index++;
    }
    return apply_replacements($value, \@replacements);
}

sub rewrite_invalid_tokens {
    my ($value) = @_;
    my @replacements;
    my $length = length($value);
    my $index = 0;

    while ($index < $length) {
        my $character = substr($value, $index, 1);
        if ($character eq "'") {
            $index = skip_single_quote($value, $index);
            next;
        }
        if ($character eq '"') {
            $index = skip_double_quote($value, $index);
            next;
        }
        if ($character eq '%') {
            $index = skip_line_comment($value, $index);
            next;
        }
        if ($character eq '/' && substr($value, $index, 2) eq '/*') {
            $index = skip_block_comment($value, $index);
            next;
        }

        if ($character =~ /[<>=+*\/-]/ ||
            substr($value, $index, 3) eq '==>' ||
            substr($value, $index, 2) eq '<=' ||
            substr($value, $index, 2) eq '>=') {
            my $end = $index + 1;
            if (substr($value, $index, 3) eq '==>') {
                $end = $index + 3;
            } else {
                $end++ while $end < $length && substr($value, $end, 1) =~ /[<>=+*\/-]/;
            }
            my $after = $end;
            if ($after < $length && substr($value, $after, 1) eq '(') {
                push @replacements, [$index, $end, quoted_atom(substr($value, $index, $end - $index))];
                $index = $end;
                next;
            }
        }

        if ($character =~ /[a-z_\$]/) {
            my $end = $index + 1;
            $end++ while $end < $length && substr($value, $end, 1) =~ /[A-Za-z0-9_\.\$]/;
            my $token = substr($value, $index, $end - $index);
            my $valid = $token =~ /^[a-z]/ ? valid_lower_word($token) : valid_defined_word($token);
            if (!$valid) {
                push @replacements, [$index, $end, quoted_atom($token)];
            }
            $index = $end;
            next;
        }

        if ($character =~ /\d/) {
            my $end = $index + 1;
            $end++ while $end < $length && substr($value, $end, 1) =~ /[A-Za-z0-9_\.\$]/;
            my $token = substr($value, $index, $end - $index);
            my $valid_number = $token =~ /^\d+$/ ||
                               $token =~ /^\d+\/\d+$/ ||
                               $token =~ /^\d+\.\d+(?:[eE][+-]?\d+)?$/ ||
                               $token =~ /^\d+[eE][+-]?\d+$/;
            if (!$valid_number && $token =~ /[_\$A-Za-z]/) {
                push @replacements, [$index, $end, quoted_atom($token)];
            }
            $index = $end;
            next;
        }

        $index++;
    }
    return apply_replacements($value, \@replacements);
}

$text = rewrite_file_sources($text);
$text = rewrite_formula_names($text);
$text = rewrite_formula_numbers($text);
$text = rewrite_generated_symbols($text);
$text = rewrite_invalid_tokens($text);

open my $out, '>', $output_path or die "write $output_path: $!\n";
print {$out} $text or die "write $output_path: $!\n";
close $out or die "close $output_path: $!\n";
PERL

    printf '[sanitize] %s -> %s\n' "${input}" "${output}"
}

for input in "${INPUTS[@]}"; do
    process_file "${input}"
done

printf '[sanitize] repaired %d proof file(s) under %s\n' "${#INPUTS[@]}" "${OUTPUT_DIR}"
