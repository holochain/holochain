/// A shrinkwrapped type with a Drop impl provided as a simple closure
#[derive(shrinkwraprs::Shrinkwrap)]
#[shrinkwrap(mutable, unsafe_ignore_visibility)]
pub struct SwanSong<T> {
    #[shrinkwrap(main_field)]
    inner: T,
    song: Option<Box<dyn FnOnce(&mut T) -> ()>>,
}

impl<T> Drop for SwanSong<T> {
    fn drop(&mut self) {
        self.song.take().unwrap()(&mut self.inner);
    }
}

impl<T> SwanSong<T> {
    pub fn new<F: FnOnce(&mut T) -> () + 'static>(inner: T, song: F) -> Self {
        Self {
            inner,
            song: Some(Box::new(song)),
        }
    }
}
