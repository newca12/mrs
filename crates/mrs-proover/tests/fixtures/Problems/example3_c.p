%------------------------------------------------------------------------------
fof(marriage, axiom,
    ! [Marriage] :
    ? [Bride] :
    ? [Groom] :
    in_love(Groom, Bride)).
fof(exists_marriage, axiom,
    is_marriage(m0)).
fof(c, conjecture,
    ? [X] :
    ? [Y] :
    in_love(X, Y)).
