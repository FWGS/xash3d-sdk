use bitflags::bitflags;

// TODO: move to client lib?
bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq)]
    pub struct KeyState: i32 {
        /// A key is not pressed.
        const NONE          = 0;
        /// A key is pressed.
        const DOWN          = 1 << 0;
        /// A key has been pressed.
        const IMPULSE_DOWN  = 1 << 1;
        /// A key has been released.
        const IMPULSE_UP    = 1 << 2;

        /// A key is pressed or has been pressed.
        const ANY_DOWN      = Self::DOWN.union(Self::IMPULSE_DOWN).bits();
    }
}
