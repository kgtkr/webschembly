use crate::VecMap;

// Noneのkeyを無視した比較を行うラッパー
#[derive(Clone)]
#[repr(transparent)]
pub struct VecMapEq<K, V>(VecMap<K, V>);

impl<K: From<usize> + Copy, V> VecMapEq<K, V>
where
    usize: From<K>,
{
    // https://github.com/rust-lang/rfcs/issues/3066
    pub fn from_ref(map: &VecMap<K, V>) -> &Self {
        unsafe { &*(map as *const VecMap<K, V> as *const VecMapEq<K, V>) }
    }

    pub fn from_mut(map: &mut VecMap<K, V>) -> &mut Self {
        unsafe { &mut *(map as *mut VecMap<K, V> as *mut VecMapEq<K, V>) }
    }

    pub fn into_inner(self) -> VecMap<K, V> {
        self.0
    }

    pub fn as_inner(&self) -> &VecMap<K, V> {
        &self.0
    }
}

impl<K, V> From<VecMap<K, V>> for VecMapEq<K, V> {
    fn from(value: VecMap<K, V>) -> Self {
        Self(value)
    }
}

impl<K, V> From<VecMapEq<K, V>> for VecMap<K, V> {
    fn from(val: VecMapEq<K, V>) -> Self {
        val.0
    }
}

impl<K: From<usize> + Copy + PartialEq, V: PartialEq> PartialEq for VecMapEq<K, V>
where
    usize: From<K>,
{
    fn eq(&self, other: &Self) -> bool {
        self.as_inner().iter().eq(other.as_inner().iter())
    }
}

impl<K: From<usize> + Copy + Eq, V: Eq> Eq for VecMapEq<K, V> where usize: From<K> {}

impl<K: From<usize> + Copy + std::hash::Hash, V: std::hash::Hash> std::hash::Hash for VecMapEq<K, V>
where
    usize: From<K>,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for (k, v) in self.as_inner().iter() {
            k.hash(state);
            v.hash(state);
        }
    }
}

impl<K: From<usize> + Copy + std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug for VecMapEq<K, V>
where
    usize: From<K>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VecMapEq")
            .field("map", self.as_inner())
            .finish()
    }
}
