//! Temp module to wrap hir_def::generics
use std::sync::Arc;

use crate::{
    db::{DefDatabase, HirDatabase},
    Adt, Const, Container, Enum, EnumVariant, Function, ImplBlock, Struct, Trait, TypeAlias, Union,
};

pub use hir_def::generics::{GenericParam, GenericParams, WherePredicate};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum GenericDef {
    Function(Function),
    Adt(Adt),
    Trait(Trait),
    TypeAlias(TypeAlias),
    ImplBlock(ImplBlock),
    // enum variants cannot have generics themselves, but their parent enums
    // can, and this makes some code easier to write
    EnumVariant(EnumVariant),
    // consts can have type parameters from their parents (i.e. associated consts of traits)
    Const(Const),
}
impl_froms!(
    GenericDef: Function,
    Adt(Struct, Enum, Union),
    Trait,
    TypeAlias,
    ImplBlock,
    EnumVariant,
    Const
);

impl GenericDef {
    pub(crate) fn resolver(&self, db: &impl HirDatabase) -> crate::Resolver {
        match self {
            GenericDef::Function(inner) => inner.resolver(db),
            GenericDef::Adt(adt) => adt.resolver(db),
            GenericDef::Trait(inner) => inner.resolver(db),
            GenericDef::TypeAlias(inner) => inner.resolver(db),
            GenericDef::ImplBlock(inner) => inner.resolver(db),
            GenericDef::EnumVariant(inner) => inner.parent_enum(db).resolver(db),
            GenericDef::Const(inner) => inner.resolver(db),
        }
    }
}

impl From<Container> for GenericDef {
    fn from(c: Container) -> Self {
        match c {
            Container::Trait(trait_) => trait_.into(),
            Container::ImplBlock(impl_block) => impl_block.into(),
        }
    }
}

pub trait HasGenericParams: Copy {
    fn generic_params(self, db: &impl DefDatabase) -> Arc<GenericParams>;
}

impl<T> HasGenericParams for T
where
    T: Into<GenericDef> + Copy,
{
    fn generic_params(self, db: &impl DefDatabase) -> Arc<GenericParams> {
        db.generic_params(self.into().into())
    }
}
