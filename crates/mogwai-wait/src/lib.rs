mod find;
mod event;

pub use find::Found;

/// Wait for
pub async fn wait_for<T, F>(
    millis: u32,
    f:F,
) -> Result<Found<T>, f64>
where
    F: Fn() -> Option<T> + 'static
{
    FoundFuture::new(millis, f).await
}


pub async fn wait(millis: u32) -> f64 {
    let future = wait_for(millis, || { None as Option<Found<()>>});
    match future.await {
        Ok(Found{elapsed,..}) => elapsed,
        Err(elapsed) => elapsed
    }
}


#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
