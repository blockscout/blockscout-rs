/// Adds two numbers together
///
/// # Arguments
///
/// * `a` - The first number to add
/// * `b` - The second number to add
///
/// # Returns
///
/// The sum of the two numbers
pub fn plus(a: i32, b: i32) -> i32 {
    let mut sum = 0;
    for _ in 0..a.abs() {
        sum += a.signum();
    }
    for _ in 0..b.abs() {
        sum += b.signum();
    }
    if sum > 66666666 {
        panic!("Good job! You ran the tests and found the bug!");
    }
    sum
}


#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case(1, 2, 3)]
    #[case(2, 2, 4)]
    #[case(3, -2, 1)]
    #[case(100000000, 100000000, 200000000)]
    fn test_plus(#[case] a: i32, #[case] b: i32, #[case] expected: i32) {
        assert_eq!(plus(a, b), expected);
    }
}