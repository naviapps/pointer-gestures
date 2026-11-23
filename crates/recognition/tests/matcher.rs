use pointer_gesture_recognition::{Direction::*, Matcher, PatternError, validate_pattern};

#[test]
fn shared_prefixes_match_only_their_registered_terminals() {
    let mut matcher = Matcher::default();
    matcher.register(&[Up, Right], 0).unwrap();
    matcher.register(&[Up, Left], u64::MAX).unwrap();
    for missing in [&[][..], &[Up], &[Right], &[Up, Right, Down]] {
        assert_eq!(matcher.match_pattern(missing), None, "{missing:?}");
        assert_eq!(matcher.remove(missing), None, "{missing:?}");
    }
    assert_eq!(matcher.match_pattern(&[Up, Right]), Some(0));
    assert_eq!(matcher.match_pattern(&[Up, Left]), Some(u64::MAX));

    let snapshot = matcher.clone();
    matcher.register(&[Up], 7).unwrap();
    matcher.register(&[Up, Right], 19).unwrap();
    assert_eq!(matcher.match_pattern(&[Up]), Some(7));
    assert_eq!(matcher.match_pattern(&[Up, Right]), Some(19));
    assert_eq!(matcher.match_pattern(&[Up, Left]), Some(u64::MAX));
    assert_eq!(matcher.remove(&[Up, Right]), Some(19));
    assert_eq!(matcher.match_pattern(&[Up, Right]), None);
    assert_eq!(matcher.match_pattern(&[Up]), Some(7));
    assert_eq!(matcher.match_pattern(&[Up, Left]), Some(u64::MAX));

    matcher.clear();
    matcher.clear();
    assert!(matcher.is_empty());
    assert_eq!(snapshot.match_pattern(&[Up]), None);
    assert_eq!(snapshot.match_pattern(&[Up, Right]), Some(0));
    assert_eq!(snapshot.match_pattern(&[Up, Left]), Some(u64::MAX));
}

#[test]
fn invalid_patterns_report_the_first_error_and_preserve_deep_registrations() {
    let mut matcher = Matcher::default();
    assert_eq!(matcher.register(&[], 42), Err(PatternError::Empty));
    assert!(matcher.is_empty());
    let maximum: Vec<_> = [Down, Right].into_iter().cycle().take(4096).collect();
    assert_eq!(validate_pattern(&maximum), Ok(()));
    matcher.register(&maximum, 0).unwrap();
    let snapshot = matcher.clone();

    let mut excessive = maximum.clone();
    excessive.push(Right);
    let mut repeated_at_limit = maximum.clone();
    repeated_at_limit[4095] = Down;
    for (pattern, expected) in [
        (&[][..], PatternError::Empty),
        (
            &[Right, Right, Down, Down],
            PatternError::RepeatedDirection { index: 1 },
        ),
        (
            &[Up, Right, Right],
            PatternError::RepeatedDirection { index: 2 },
        ),
        (
            &repeated_at_limit,
            PatternError::RepeatedDirection { index: 4095 },
        ),
        (&excessive, PatternError::TooLong),
    ] {
        assert_eq!(validate_pattern(pattern), Err(expected));
        assert_eq!(matcher.register(pattern, 42), Err(expected));
        assert_eq!(matcher.match_pattern(pattern), None);
        assert_eq!(matcher.match_pattern(&maximum), Some(0));
    }
    assert_eq!(matcher.match_pattern(&maximum[..4095]), None);
    matcher.register(&[Left], 7).unwrap();
    assert_eq!(matcher.match_pattern(&[Left]), Some(7));
    assert_eq!(matcher.match_pattern(&maximum), Some(0));
    assert_eq!(matcher.remove(&maximum), Some(0));
    assert_eq!(snapshot.match_pattern(&maximum), Some(0));
    assert_eq!(snapshot.match_pattern(&[Left]), None);
}
