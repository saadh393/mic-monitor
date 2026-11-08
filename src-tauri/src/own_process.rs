pub fn without(pids: Vec<i32>, own: i32) -> Vec<i32> {
    pids.into_iter().filter(|&pid| pid != own).collect()
}

#[cfg(test)]
mod tests {
    use super::without;

    #[test]
    fn removes_own_pid() {
        assert_eq!(without(vec![10, 42, 7], 42), vec![10, 7]);
    }

    #[test]
    fn removes_every_copy() {
        assert_eq!(without(vec![42, 10, 42], 42), vec![10]);
    }

    #[test]
    fn keeps_list_without_own_pid() {
        assert_eq!(without(vec![10, 7], 42), vec![10, 7]);
        assert_eq!(without(vec![], 42), Vec::<i32>::new());
    }
}
