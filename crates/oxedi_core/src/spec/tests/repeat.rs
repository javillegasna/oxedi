//! Which element positions a repeated group holds.

use crate::spec::Repeat;

#[test]
fn a_repeat_holds_its_offset_in_every_group_from_the_first() {
    let repeat = Repeat { from: 3, step: 2 };
    assert_eq!(
        (1..=9).filter(|&p| repeat.holds(1, p)).collect::<Vec<_>>(),
        [4, 6, 8]
    );
    assert_eq!(
        (1..=9).filter(|&p| repeat.holds(0, p)).collect::<Vec<_>>(),
        [3, 5, 7, 9]
    );
    let single = Repeat { from: 2, step: 0 };
    assert_eq!(
        (1..=5).filter(|&p| single.holds(0, p)).collect::<Vec<_>>(),
        [2]
    );
    assert!(
        !Repeat {
            from: usize::MAX,
            step: 2
        }
        .holds(1, 5)
    );
}

#[test]
fn a_repeat_holds_the_groups_up_to_its_last_element() {
    let repeat = Repeat { from: 2, step: 3 };
    assert_eq!(repeat.capacity(Some(19)), 6);
    assert_eq!(repeat.capacity(Some(2)), 1);
    assert_eq!(repeat.capacity(Some(1)), 1);
    assert_eq!(repeat.capacity(None), 1);
}
