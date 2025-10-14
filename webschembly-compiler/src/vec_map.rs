use std::marker::PhantomData;

use crate::HasId;

// 削除可能なTiVec likeな構造
// キーの値が大きすぎず、密であることが期待される場合に有効
// 例: VecMap<LocalId, Local>, VecMap<BasicBlockId, BasicBlock>
#[cfg_attr(test, derive(PartialEq, Eq))]
#[derive(Clone)]
pub struct VecMap<K, V> {
    vec: Vec<Option<V>>,
    _marker: PhantomData<fn(K) -> K>,
}
// PartialEq, Eq, Hashはallocate_keyにより非直感的な動作をするのでcfg_attrをつけて実装

impl<K: From<usize> + Copy, V> Default for VecMap<K, V>
where
    usize: From<K>,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<K: From<usize> + Copy, V> VecMap<K, V>
where
    usize: From<K>,
{
    pub fn new() -> Self {
        Self {
            vec: Vec::new(),
            _marker: PhantomData,
        }
    }

    pub fn entry(&mut self, key: K) -> Entry<'_, K, V> {
        if self.get(key).is_some() {
            Entry::Occupied(OccupiedEntry { key, map: self })
        } else {
            Entry::Vacant(VacantEntry { key, map: self })
        }
    }

    pub fn insert(&mut self, key: K, value: V) {
        let i = usize::from(key);
        if i >= self.vec.len() {
            self.vec.resize_with(i + 1, || None);
        }
        self.vec[i] = Some(value);
    }

    pub fn get(&self, key: K) -> Option<&V> {
        let i = usize::from(key);
        self.vec.get(i).and_then(|opt| opt.as_ref())
    }

    pub fn contains_key(&self, key: K) -> bool {
        self.get(key).is_some()
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        let i = usize::from(key);
        self.vec.get_mut(i).and_then(|opt| opt.as_mut())
    }

    pub fn remove(&mut self, key: K) -> Option<V> {
        let i = usize::from(key);
        if i >= self.vec.len() {
            return None;
        }
        self.vec[i].take()
    }

    pub fn keys(&self) -> impl Iterator<Item = K> {
        self.vec
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|_| K::from(i)))
    }

    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.vec.iter().filter_map(|v| v.as_ref())
    }

    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.vec.iter_mut().filter_map(|v| v.as_mut())
    }

    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> {
        self.vec
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|v| (K::from(i), v)))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> {
        self.vec
            .iter_mut()
            .enumerate()
            .filter_map(|(i, v)| v.as_mut().map(|v| (K::from(i), v)))
    }

    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(K, &mut V) -> bool,
    {
        for (i, v) in self.vec.iter_mut().enumerate() {
            if let Some(v2) = v
                && !f(K::from(i), v2)
            {
                *v = None;
            }
        }
    }

    pub fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (k, v) in iter {
            self.insert(k, v);
        }
    }

    // O(n)なのでlen/is_emptyは実装しない

    // push_withなどを使うべきなのでprivate
    fn next_key(&self) -> K {
        K::from(self.vec.len())
    }

    pub fn allocate_key(&mut self) -> K {
        let key = self.next_key();
        self.vec.push(None);
        key
    }

    pub fn push(&mut self, value: V) -> K {
        let key = self.next_key();
        self.vec.push(Some(value));
        key
    }

    pub fn push_with<F: FnOnce(K) -> V>(&mut self, f: F) -> K {
        let key = self.next_key();
        let value = f(key);
        self.vec.push(Some(value));
        key
    }
}

impl<K: From<usize> + Copy, V> std::ops::Index<K> for VecMap<K, V>
where
    usize: From<K>,
{
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index).expect("no entry found for key")
    }
}

impl<K: From<usize> + Copy, V> std::ops::IndexMut<K> for VecMap<K, V>
where
    usize: From<K>,
{
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.get_mut(index).expect("no entry found for key")
    }
}

impl<K: From<usize> + Copy, V> FromIterator<(K, V)> for VecMap<K, V>
where
    usize: From<K>,
{
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

impl<K: From<usize> + Copy, V> IntoIterator for VecMap<K, V>
where
    usize: From<K>,
{
    type Item = (K, V);
    type IntoIter = impl Iterator<Item = (K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.vec
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (K::from(i), v)))
    }
}

// HasIdを実装している型をnodeと呼ぶ
impl<K: From<usize> + Copy, V: HasId<Id = K>> VecMap<K, V>
where
    usize: From<K>,
{
    pub fn insert_node(&mut self, value: V) {
        let key = value.id();
        self.insert(key, value);
    }

    pub fn from_nodes<I: IntoIterator<Item = V>>(iter: I) -> Self {
        let mut map = Self::new();
        for v in iter {
            map.insert_node(v);
        }
        map
    }
}

impl<K: From<usize> + Copy, V: HasId<Id = K>> FromIterator<V> for VecMap<K, V>
where
    usize: From<K>,
{
    fn from_iter<I: IntoIterator<Item = V>>(iter: I) -> Self {
        Self::from_nodes(iter)
    }
}

pub struct OccupiedEntry<'a, K, V> {
    key: K,
    map: &'a mut VecMap<K, V>,
}

impl<'a, K: From<usize> + Copy + 'a, V: 'a> OccupiedEntry<'a, K, V>
where
    usize: From<K>,
{
    pub fn get(&self) -> &V {
        &self.map[self.key]
    }

    pub fn get_mut(&mut self) -> &mut V {
        &mut self.map[self.key]
    }

    pub fn into_mut(self) -> &'a mut V {
        self.map.get_mut(self.key).unwrap()
    }

    pub fn remove(self) -> V {
        self.map.remove(self.key).unwrap()
    }

    pub fn insert(&mut self, value: V) -> V {
        std::mem::replace(self.get_mut(), value)
    }
}

pub struct VacantEntry<'a, K, V> {
    key: K,
    map: &'a mut VecMap<K, V>,
}

impl<'a, K: From<usize> + Copy + 'a, V: 'a> VacantEntry<'a, K, V>
where
    usize: From<K>,
{
    pub fn insert(self, value: V) -> &'a mut V {
        self.map.insert(self.key, value);
        self.map.get_mut(self.key).unwrap()
    }
}

pub enum Entry<'a, K, V> {
    Occupied(OccupiedEntry<'a, K, V>),
    Vacant(VacantEntry<'a, K, V>),
}

impl<'a, K: From<usize> + Copy + 'a, V: 'a> Entry<'a, K, V>
where
    usize: From<K>,
{
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default),
        }
    }

    pub fn or_insert_with(self, default: impl FnOnce() -> V) -> &'a mut V {
        match self {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => entry.insert(default()),
        }
    }
}

impl<K: From<usize> + Copy + std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug for VecMap<K, V>
where
    usize: From<K>,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_map = f.debug_map();
        debug_map.entries(self.iter());
        if let Some(None) = self.vec.last() {
            debug_map.finish_non_exhaustive()
        } else {
            debug_map.finish()
        }
    }
}

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
