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
}

impl<K, V> std::ops::Deref for VecMapEq<K, V> {
    type Target = VecMap<K, V>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> std::ops::DerefMut for VecMapEq<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<K, V> From<VecMap<K, V>> for VecMapEq<K, V> {
    fn from(value: VecMap<K, V>) -> Self {
        Self(value)
    }
}

impl<K, V> Into<VecMap<K, V>> for VecMapEq<K, V> {
    fn into(self) -> VecMap<K, V> {
        self.0
    }
}

impl<K: From<usize> + Copy + PartialEq, V: PartialEq> PartialEq for VecMapEq<K, V>
where
    usize: From<K>,
{
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}

impl<K: From<usize> + Copy + Eq, V: Eq> Eq for VecMapEq<K, V> where usize: From<K> {}

impl<K: From<usize> + Copy + std::hash::Hash, V: std::hash::Hash> std::hash::Hash for VecMapEq<K, V>
where
    usize: From<K>,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        for (k, v) in self.iter() {
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
        f.debug_struct("VecMapEq").field("map", &self.0).finish()
    }
}
