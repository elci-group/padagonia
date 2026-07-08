use serde::{Deserialize, Serialize};
use std::fmt::{self, Display};

macro_rules! id_u64 {
    ($name:ident) => {
        #[derive(
            Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u64);

        impl $name {
            pub const fn new(v: u64) -> Self {
                Self(v)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

macro_rules! id_u32 {
    ($name:ident) => {
        #[derive(
            Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(pub u32);

        impl $name {
            pub const fn new(v: u32) -> Self {
                Self(v)
            }
        }

        impl Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, concat!(stringify!($name), "({})"), self.0)
            }
        }
    };
}

id_u64!(NodeId);
id_u64!(EdgeId);
id_u64!(FactId);
id_u32!(LabelId);
id_u32!(RelationId);
id_u32!(KeyId);
