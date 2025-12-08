use vstd::prelude::*;

verus! {

pub fn get_safe(arr: &[u32], index: usize) -> (result: Option<u32>)
    ensures
        index < arr.len() ==> result == Some(arr[index as int]),
        index >= arr.len() ==> result matches None,
{
    if index < arr.len() {
        Some(arr[index])
    } else {
        None
    }
}

// Simplified sum function without complex invariant
pub fn sum_two(a: u32, b: u32) -> (result: u64)
    requires
        a <= 1000000,
        b <= 1000000,
    ensures
        result == (a as u64) + (b as u64),
{
    (a as u64) + (b as u64)
}

pub fn binary_search(arr: &[u32], target: u32) -> (result: Option<usize>)
    requires
        is_sorted(arr@),
    ensures
        match result {
            Some(idx) => {
                idx < arr.len() &&
                arr[idx as int] == target
            },
            None => {
                forall|i: int| 0 <= i < arr.len() ==> arr[i] != target
            },
        },
{
    let mut low: usize = 0;
    let mut high: usize = arr.len();

    while low < high
        invariant
            low <= high,
            high <= arr.len(),
            is_sorted(arr@),
            forall|i: int| 0 <= i < low ==> arr[i] < target,
            forall|i: int| high <= i < arr.len() ==> arr[i] > target,
        decreases high - low,
    {
        let mid = low + (high - low) / 2;

        if arr[mid] == target {
            return Some(mid);
        } else if arr[mid] < target {
            low = mid + 1;
        } else {
            high = mid;
        }
    }

    None
}

pub open spec fn is_sorted(s: Seq<u32>) -> bool {
    forall|i: int, j: int| 0 <= i < j < s.len() ==> s[i] <= s[j]
}

pub struct BoundedVec {
    pub data: Vec<u32>,
    pub capacity: usize,
}

impl BoundedVec {
    pub open spec fn inv(&self) -> bool {
        self.data@.len() <= self.capacity
    }

    pub fn new(capacity: usize) -> (result: BoundedVec)
        requires
            capacity > 0,
            capacity <= 1000,
        ensures
            result.capacity == capacity,
            result.data@.len() == 0,
            result.inv(),
    {
        BoundedVec {
            data: Vec::new(),
            capacity,
        }
    }

    pub fn push(&mut self, value: u32) -> (result: bool)
        requires
            old(self).inv(),
        ensures
            result == (old(self).data@.len() < old(self).capacity),
            result ==> {
                self.data@.len() == old(self).data@.len() + 1 &&
                self.data@[self.data@.len() - 1] == value &&
                forall|i: int| 0 <= i < old(self).data@.len() ==>
                    self.data@[i] == old(self).data@[i]
            },
            !result ==> self.data@ == old(self).data@,
            self.capacity == old(self).capacity,
            self.inv(),
    {
        if self.data.len() < self.capacity {
            self.data.push(value);
            true
        } else {
            false
        }
    }

    pub fn pop(&mut self) -> (result: Option<u32>)
        requires
            old(self).inv(),
        ensures
            old(self).data@.len() == 0 ==> result matches None,
            old(self).data@.len() > 0 ==> {
                result == Some(old(self).data@[old(self).data@.len() - 1]) &&
                self.data@.len() == old(self).data@.len() - 1 &&
                self.data@ == old(self).data@.subrange(0, old(self).data@.len() - 1)
            },
            self.capacity == old(self).capacity,
            self.inv(),
    {
        self.data.pop()
    }

    pub fn len(&self) -> (result: usize)
        requires
            self.inv(),
        ensures
            result == self.data@.len(),
            result <= self.capacity,
    {
        self.data.len()
    }

    pub fn is_empty(&self) -> (result: bool)
        ensures
            result == (self.data@.len() == 0),
    {
        self.data.len() == 0
    }
}

pub fn max(a: u32, b: u32) -> (result: u32)
    ensures
        result >= a,
        result >= b,
        result == a || result == b,
{
    if a > b { a } else { b }
}

pub fn is_even(n: u32) -> (result: bool)
    ensures
        result == (n % 2 == 0),
{
    n % 2 == 0
}

// Simplified factorial for small values
pub fn factorial_small(n: u8) -> (result: u64)
    requires
        n <= 5,
    ensures
        result > 0,
{
    if n == 0 {
        1
    } else if n == 1 {
        1
    } else if n == 2 {
        2
    } else if n == 3 {
        6
    } else if n == 4 {
        24
    } else {
        120
    }
}

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_safe() {
        let arr = [1, 2, 3, 4, 5];
        assert_eq!(get_safe(&arr, 0), Some(1));
        assert_eq!(get_safe(&arr, 4), Some(5));
        assert_eq!(get_safe(&arr, 5), None);
    }

    #[test]
    fn test_binary_search() {
        let arr = [1, 3, 5, 7, 9, 11, 13];
        assert_eq!(binary_search(&arr, 7), Some(3));
        assert_eq!(binary_search(&arr, 1), Some(0));
        assert_eq!(binary_search(&arr, 13), Some(6));
        assert_eq!(binary_search(&arr, 4), None);
        assert_eq!(binary_search(&arr, 0), None);
    }

    #[test]
    fn test_bounded_vec() {
        let mut vec = BoundedVec::new(3);
        assert_eq!(vec.len(), 0);
        assert!(vec.is_empty());

        assert!(vec.push(10));
        assert!(vec.push(20));
        assert!(vec.push(30));
        assert!(!vec.push(40));

        assert_eq!(vec.len(), 3);
        assert!(!vec.is_empty());

        assert_eq!(vec.pop(), Some(30));
        assert_eq!(vec.pop(), Some(20));
        assert_eq!(vec.len(), 1);

        assert!(vec.push(40));
        assert_eq!(vec.len(), 2);
    }

    #[test]
    fn test_max() {
        assert_eq!(max(5, 10), 10);
        assert_eq!(max(10, 5), 10);
        assert_eq!(max(7, 7), 7);
    }

    #[test]
    fn test_is_even() {
        assert!(is_even(0));
        assert!(is_even(2));
        assert!(is_even(100));
        assert!(!is_even(1));
        assert!(!is_even(99));
    }

    #[test]
    fn test_factorial() {
        assert_eq!(factorial_small(0), 1);
        assert_eq!(factorial_small(1), 1);
        assert_eq!(factorial_small(5), 120);
    }

    #[test]
    fn test_sum_two() {
        assert_eq!(sum_two(100, 200), 300);
        assert_eq!(sum_two(0, 0), 0);
        assert_eq!(sum_two(1000000, 1000000), 2000000);
    }
}
