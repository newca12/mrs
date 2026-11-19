%----SIMPLE CASE
%----Standard S5 version 1
% Standard S5 semantics are specified by both simple_s5 variants below.
thf(simple_s5,logic,(
    $modal ==
      [ $logicfile == 'LOG002_1.l',
        $constants == $rigid,
        $quantification == $constant,
        $consequence == $global,
        $modalities == $modal_system_S5 ] )).

thf(standard_s5_example,axiom,({#box} @ p => p)).

thf(p_type,type,(p: $o)).
thf(q_type,type,(q: $o)).

%----Standard S5 version 2
%----Default modality S5 as list of axioms K + T + 5.
thf(simple_s5_v3,logic,(
    $modal ==
      [ $constants == $rigid,
        $quantification == $constant,
        $consequence == $global,
        $modalities ==
          [ $modal_axiom_K,
            $modal_axiom_T,
            $modal_axiom_5 ] ] )).


%----CONSTANTS OPTIONS
% king_of_france is flexible; socrates uses the default rigid-constant setting.

%----Constants The constant king_of_france has special semantics
thf(human_type,type,(
    human: $tType )).

thf(king_of_france_constant,type,(
    king_of_france: human )).

thf(socrates_constant,type,(
    socrates: human )).

thf(person_type,type,(
    person: human > $o )).

thf(constants,logic,(
    $modal ==
      [ $logicfile == 'LOG002_1.l',
        $constants ==
          [ $constant,
            king_of_france == $flexible ],
        $quantification == $constant,
        $consequence == $global,
        $modalities == $modal_system_S5 ] )).

thf(constants_example,axiom,(
    person @ king_of_france & person @ socrates )).


%----QUANTIFICATION OPTIONS
% simple_s5 and constants specify constant-domain quantification.
% Example problem with at least one cumulative domain: PUZ149*.p
% Example problem with at least one varying domain: PUZ087*.p
% The quantification specification below combines constant and varying domains.

%----Quantification may be different for any base type or compound type e.g.
%----for type plushie
thf(plushie_type,type,(
    plushie: $tType )).

thf(toy_type,type,(
    toy: plushie > $o )).

thf(quantification,logic,(
    $modal ==
      [ $logicfile == 'LOG002_1.l',
        $constants == $rigid,
        $quantification ==
          [ $constant,
            plushie == $varying ],
        $consequence == $global,
        $modalities == $modal_system_S5 ] )).

thf(domain_options_example,axiom,(
    (![X:human] : (person @ X)) & (![Y:plushie] : (toy @ Y)) )).


%----CONSEQUENCE OPTIONS
% ax1 and local_goal are local; global_axiom and global_goal use the default.

%----Consequence. All axioms and the conjecture have the same consequence 
%----except for ax1 which has a special consequence
thf(ax1,axiom,(
    $true )).

thf(global_axiom,axiom,(
    $true )).

thf(local_goal,conjecture,(
    $true )).

thf(global_goal,conjecture,(
    $true )).

thf(different_consequence,logic,(
    $modal ==
      [ $logicfile == 'LOG002_1.l',
        $constants == $rigid,
        $quantification == $constant,
        $consequence ==
          [ $global,
            ax1 == $local,
            local_goal == $local ],
        $modalities == $modal_system_S5 ] )).


%----MODALITY OPTIONS
% The a and b modalities below override the default S5 modality differently.

%----Special semantics for modalities with index a and b (here: systems KB and 
%----K); one default value for all other modalities (here: system S5)
thf(exotic,logic,(
    $modal ==
      [ $constants == $flexible,
        $quantification == $cumulative,
        $consequence == $local,
        $modalities ==
          [ $modal_system_S5,
            a == $modal_system_KB,
            b == $modal_system_K ] ] )).

thf(multiple_modalities_example,axiom,(
    ({#a} @ p) & ({#b} @ q) )).
