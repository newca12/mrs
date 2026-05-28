%------------------------------------------------------------------------------
% Proof : Problems/example3_e.p
%------------------------------------------------------------------------------
fof(marriage, axiom,
    ! [Marriage] :
    ? [Bride] :
    ? [Groom] :
    in_love(Groom, Bride), file('Problems/example3_e.p',marriage)).
fof(exists_marriage, axiom,
    is_marriage(m0), file('Problems/example3_e.p',exists_marriage)).
fof(c, conjecture,
    ? [X] :
    ? [Y] :
    in_love(X, Y), file('Problems/example3_e.p',c)).
fof(neg_c, negated_conjecture,
    ~(? [X] :
    ? [Y] :
    in_love(X, Y)), inference(negated_conjecture, [status(cth)], [c])).
fof(bride,plain,
    ! [Marriage] :
    ? [Groom] :
      in_love(Groom,sK0(Marriage)),
    inference(skolemize, [status(esa), new_symbols(skolem, [sK0]), skolemize(Bride, sK0(Marriage))], [marriage])).
fof(groom,plain,
    ! [Marriage] :
      in_love(sK0(Marriage),sK0(Marriage)),
    inference(skolemize,[status(esa), new_symbols(skolem, [sK0]), skolemize(Groom, sK0(Marriage))], [bride])).
fof(groom_m0, plain,
    in_love(sK0(m0), sK0(m0)), inference(instantiate, [status(thm)], [groom])).
fof(contradiction, plain,
    $false,
    inference(consequence, [status(thm)], [neg_c, groom_m0])).
