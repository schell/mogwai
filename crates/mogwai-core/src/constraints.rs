//! Feature gated Send + Sync constraints and marker traits.

#[cfg(feature = "send")]
pub trait SendConstraints: Send + 'static {}
#[cfg(feature = "send")]
impl<T: Send + 'static> SendConstraints for T {}

#[cfg(not(feature = "send"))]
pub trait SendConstraints: 'static {}
#[cfg(not(feature = "send"))]
impl<T: 'static> SendConstraints for T {}

#[cfg(feature = "sync")]
pub trait SyncConstraints: Sync + 'static {}
#[cfg(feature = "sync")]
impl<T: Sync + 'static> SyncConstraints for T {}

#[cfg(not(feature = "sync"))]
pub trait SyncConstraints: 'static {}
#[cfg(not(feature = "sync"))]
impl<T: 'static> SyncConstraints for T {}

pub trait Spawnable<T>: futures::Future<Output = T> + SendConstraints + SyncConstraints {}
impl<T, F: futures::Future<Output = T> + SendConstraints + SyncConstraints> Spawnable<T> for F {}
