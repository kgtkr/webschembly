use bimap::BiHashMap;
use rustc_hash::FxBuildHasher;

pub type FxBiHashMap<K, V> = BiHashMap<K, V, FxBuildHasher>;
