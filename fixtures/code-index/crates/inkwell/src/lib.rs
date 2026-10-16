//! The inkwell library: sheets of marks (SLAB-SHEET).
//! @implements SLAB-SHEET@1

pub mod broken;
pub mod page;
pub mod split;
pub mod twins;

/// Outline of a sheet.
pub mod outline {
    //! The outline, documented inside its own braces.
    //! @implements TENET-ORDER@1

    pub mod nested {
        //! Nested one level deeper (TENET-ORDER).

        // @implements TENET-WIDTH@2
        pub fn depth() -> u32 {
            let first = 1;
            first + 1
        }
    }
}
