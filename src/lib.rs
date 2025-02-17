use std::{
    ops::{Deref, DerefMut, Index, IndexMut, Range},
    slice,
    slice::SliceIndex,
    sync::Arc, fmt::Display,
};

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Snap<T> {
    buf: Arc<Vec<T>>,
    range: Range<usize>,
}

impl<T> Snap<T> {
    #[inline]
    pub fn new(vec: Vec<T>) -> Self {
        let range = 0..vec.len();
        let buf = Arc::new(vec);

        Snap { buf, range }
    }

    #[inline]
    pub fn snap(self, at: usize) -> (Self, Self) {
        assert!((0..=self.len()).contains(&at), "`snap`-ing out of range");

        let left_buf = self.buf.clone();
        let left_range = self.range.start..(self.range.start + at);

        let right_buf = self.buf;
        let right_range = (self.range.start + at)..self.range.end;

        (
            Snap {
                buf: left_buf,
                range: left_range,
            },
            Snap {
                buf: right_buf,
                range: right_range,
            },
        )
    }

    #[inline]
    pub fn merge(left: Self, right: Self) -> Self {
        assert!(
            left.buf.as_ptr() == right.buf.as_ptr(),
            "merging `Snaps` of different origins"
        );

        assert!(
            left.range.end == right.range.start,
            "merging non-continuogus `Snaps`"
        );

        let buf = left.buf;
        let range = left.range.start..right.range.end;

        Snap { buf, range }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.range.len()
    }

    #[inline]
    pub fn is_complete(&self) -> bool {
        self.range == (0..self.buf.len())
    }

    #[inline]
    pub fn range(&self) -> &Range<usize> {
        &self.range
    }

    #[inline]
    pub fn get<I: SliceIndex<[T]>>(&self, index: I) -> Option<&<I as SliceIndex<[T]>>::Output> {
        self.as_slice().get(index)
    }

    #[inline]
    pub fn get_mut<I: SliceIndex<[T]>>(
        &mut self,
        index: I,
    ) -> Option<&mut <I as SliceIndex<[T]>>::Output> {
        self.as_mut_slice().get_mut(index)
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.buf[self.range.clone()]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        let ptr = self.buf[self.range.clone()].as_ptr() as *mut T;
        let len = self.len();

        unsafe { slice::from_raw_parts_mut(ptr, len) }
    }

    #[inline]
    pub fn chunks(mut self, chunk_size: usize) -> Vec<Self> {
        let mut chunks = Vec::with_capacity((self.len() + chunk_size - 1) / chunk_size);

        while self.len() > chunk_size {
            self = {
                let (chunk, tail) = self.snap(chunk_size);
                chunks.push(chunk);
                tail
            };
        }

        chunks.push(self);

        chunks
    }

    #[inline]
    pub fn try_unwrap(self) -> Result<Vec<T>, Self> {
        match Arc::try_unwrap(self.buf) {
            Ok(vec) => Ok(vec),
            Err(arc) => Err(Snap {
                buf: arc,
                range: self.range,
            }),
        }
    }
}

impl<T, I: SliceIndex<[T]>> Index<I> for Snap<T> {
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &Self::Output {
        Index::index(self.as_slice(), index)
    }
}

impl<T, I: SliceIndex<[T]>> IndexMut<I> for Snap<T> {
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        IndexMut::index_mut(self.as_mut_slice(), index)
    }
}

impl<'a, T> IntoIterator for &'a Snap<T> {
    type Item = &'a T;
    type IntoIter = slice::Iter<'a, T>;

    fn into_iter(self) -> slice::Iter<'a, T> {
        self.as_slice().into_iter()
    }
}

impl<'a, T> IntoIterator for &'a mut Snap<T> {
    type Item = &'a mut T;
    type IntoIter = slice::IterMut<'a, T>;

    fn into_iter(self) -> slice::IterMut<'a, T> {
        self.as_mut_slice().into_iter()
    }
}

impl<T> Deref for Snap<T> {
    type Target = [T];

    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> DerefMut for Snap<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<T> Clone for Snap<T> {
    fn clone(&self) -> Self {
        Snap {
            buf: self.buf.clone(),
            range: self.range.clone(),
        }
    }
}

impl<T> Display for Snap<T>
where
    T: Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[")?;

        for (i, item) in self.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }

            write!(f, "{}", item)?;
        }

        write!(f, "]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, right) = snap.snap(2);

        assert_eq!(left[..], [0, 1]);
        assert_eq!(right[..], [2, 3]);
    }

    #[test]
    fn double_snap() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, right) = snap.snap(2);
        let (left, middle) = left.snap(1);

        assert_eq!(left[..], [0]);
        assert_eq!(middle[..], [1]);
        assert_eq!(right[..], [2, 3]);
    }

    #[test]
    fn snap_empty_left() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (_, snap) = snap.snap(4);
        assert_eq!(snap[..], []);
    }

    #[test]
    fn snap_empty_right() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (snap, _) = snap.snap(0);
        assert_eq!(snap[..], []);
    }

    #[test]
    #[should_panic(expected = "`snap`-ing out of range")]
    fn snap_out_of_range() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        snap.snap(8);
    }

    #[test]
    fn merge() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, right) = snap.snap(2);

        let snap = Snap::merge(left, right);
        assert_eq!(snap[..], [0, 1, 2, 3]);
    }

    #[test]
    #[should_panic(expected = "merging `Snaps` of different origins")]
    fn merge_snaps_different_origins() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let snap_prime = Snap::new(vec![0, 1, 2, 3]);

        let (left, _right) = snap.snap(2);
        let (_left, right) = snap_prime.snap(2);
        Snap::merge(left, right);
    }

    #[test]
    #[should_panic]
    fn merge_swapped() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, right) = snap.snap(2);
        let _ = Snap::merge(right, left);
    }

    #[test]
    #[should_panic]
    fn merge_gap() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, right) = snap.snap(2);
        let (left, _) = left.snap(1);
        let _ = Snap::merge(left, right);
    }

    #[test]
    fn len() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        assert_eq!(snap.len(), 4);

        let (left, right) = snap.snap(2);
        assert_eq!(left.len(), 2);
        assert_eq!(right.len(), 2);
    }

    #[test]
    fn is_complete() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        assert!(snap.is_complete());

        let (left, right) = snap.snap(2);
        assert!(!left.is_complete());
        assert!(!right.is_complete());

        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (snap, _) = snap.snap(4);
        assert!(snap.is_complete());
    }

    #[test]
    fn range() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, right) = snap.snap(2);
        assert_eq!(*left.range(), 0..2);
        assert_eq!(*right.range(), 2..4);
    }

    #[test]
    fn get() {
        let snap = Snap::new(vec![0, 1, 2, 3]);

        for i in 0..3 {
            assert_eq!(snap.get(i), Some(&[0, 1, 2, 3][i]));
        }
    }

    #[test]
    fn get_non_existant() {
        let snap = Snap::new(vec![0, 1, 2, 3]);

        for i in 4..9 {
            assert_eq!(snap.get(i), None);
        }
    }

    #[test]
    fn get_mut() {
        let mut snap = Snap::new(vec![0, 1, 2, 3]);

        for i in 0..3 {
            *snap.get_mut(i).unwrap() += 1;
            assert_eq!(snap[i], [1, 2, 3, 4][i]);
        }
    }

    #[test]
    fn unwrap() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let vec = snap.try_unwrap();
        assert!(vec.is_ok());
    }

    #[test]
    fn unwrap_snapped() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        let (left, _right) = snap.snap(2);

        let vec = left.try_unwrap();
        assert!(vec.is_err());
    }

    #[test]
    fn iter() {
        let snap = Snap::new(vec![0, 1, 2, 3]);
        assert!(snap.iter().zip([0, 1, 2, 3].iter()).all(|(l, r)| l == r));
    }

    #[test]
    fn iter_mut() {
        let mut snap = Snap::new(vec![0, 1, 2, 3]);

        for i in snap.iter_mut() {
            *i += 1;
        }

        assert!(snap.iter().zip([1, 2, 3, 4].iter()).all(|(l, r)| l == r));
    }

    #[test]
    fn chunks_balanced() {
        let snap = Snap::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        let chunks = snap.chunks(4);

        assert_eq!(chunks.len(), 4);

        assert_eq!(*chunks[0].range(), 0..4);
        assert_eq!(*chunks[1].range(), 4..8);
        assert_eq!(*chunks[2].range(), 8..12);
        assert_eq!(*chunks[3].range(), 12..16);
    }

    #[test]
    fn chunks_unbalanced() {
        let snap = Snap::new(vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
        let chunks = snap.chunks(5);

        assert_eq!(chunks.len(), 4);

        assert_eq!(*chunks[0].range(), 0..5);
        assert_eq!(*chunks[1].range(), 5..10);
        assert_eq!(*chunks[2].range(), 10..15);
        assert_eq!(*chunks[3].range(), 15..16);
    }
}
