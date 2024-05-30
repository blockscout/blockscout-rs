//! To simplify implementation of overly-generic `DataSource` trait
//! as well as to reduce code duplication, traits in this module were
//! created.
//!
//! ## Choosing a trait
//!
//! It is generally easier to implement the most specific trait one can find.
//! Thus, see traits within this module to find the suitable one.
//!
//! ## Using a trait
//!
//! See [`crate::data_source`, section "Implementation"](crate::data_source)
//!
//! ## Creating a new trait
//!
//! Let's say you want to create trait `TraitName`
//! 1. Find the most specific trait that fits the use case (according to `Choosing a trait` section above).
//! Let's say this trait is called `ParentTrait` (it can be even `DataSource`).
//! 2. Define `TraitName`
//! 3. Create newtype `TraitNameLocalWrapper<T: TraitName>` that's going to wrap types that implement `TraitName`.
//! 4. Write `impl ParentTrait for TraitNameWrapper`
//! 5. Create type alias `TraitWrapper` to simplify getting `DataSource` from the types.
//! 6. (for charts) (option) create delegated `Chart` implementation `impl<..> Chart for TraitNameWrapper<C> {}`
//! (see existing impls for example)
//! 7. Check if you need to somehow collect respective metrics
//! ...some other stuff I forgor...
//!

pub mod adapter;
pub mod remote;
pub mod updateable_chart;
