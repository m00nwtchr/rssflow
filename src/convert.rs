use async_trait::async_trait;

#[async_trait]
pub trait AsyncFrom<T> {
	async fn from_async(value: T) -> Self;
}

#[async_trait]
impl<T: Send + Sync> AsyncFrom<T> for T {
	async fn from_async(value: T) -> Self {
		value
	}
}

#[async_trait]
pub trait AsyncInto<T>: Sized {
	/// Converts this type into the (usually inferred) input type.
	#[must_use]
	async fn into_async(self) -> T;
}

#[async_trait]
impl<T: Send + Sync, U> AsyncInto<U> for T
where
	U: AsyncFrom<T>,
{
	/// Calls `U::from_async(self)`.
	///
	/// That is, this conversion is whatever the implementation of
	/// <code>[AsyncFrom]&lt;T&gt; for U</code> chooses to do.
	#[inline]
	#[track_caller]
	async fn into_async(self) -> U {
		U::from_async(self).await
	}
}

#[async_trait]
pub trait AsyncTryFrom<T>: Sized {
	/// The type returned in the event of a conversion error.
	type Error;

	/// Performs the conversion.
	async fn try_from_async(value: T) -> Result<Self, Self::Error>;
}

#[async_trait]
pub trait AsyncTryInto<T>: Sized {
	/// The type returned in the event of a conversion error.
	type Error;

	/// Performs the conversion.
	async fn try_into_async(self) -> Result<T, Self::Error>;
}

#[async_trait]
impl<T: Sync + Send, U> AsyncTryInto<U> for T
where
	U: AsyncTryFrom<T>,
{
	type Error = U::Error;

	#[inline]
	async fn try_into_async(self) -> Result<U, U::Error> {
		U::try_from_async(self).await
	}
}
