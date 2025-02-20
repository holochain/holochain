/// A shrinkwrapped type with a Drop impl provided as a simple closure
#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct SwanSong<'a, T> {
    #[shrinkwrap(main_field)]
    inner: T,
    #[allow(clippy::type_complexity)]
    song: Option<Box<dyn FnOnce(&mut T) + 'a>>,
}

impl<T> Drop for SwanSong<'_, T> {
    fn drop(&mut self) {
        self.song.take().unwrap()(&mut self.inner);
    }
}

impl<'a, T> SwanSong<'a, T> {
    pub fn new<F: FnOnce(&mut T) + 'a>(inner: T, song: F) -> Self {
        Self {
            inner,
            song: Some(Box::new(song)),
        }
    }
}
