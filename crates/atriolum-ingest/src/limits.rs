/// Maximum compressed envelope size (20 MB).
pub const MAX_COMPRESSED_SIZE: usize = 20 * 1024 * 1024;

/// Maximum decompressed envelope size (100 MB).
pub const MAX_DECOMPRESSED_SIZE: usize = 100 * 1024 * 1024;

/// Maximum event/transaction item size (1 MB).
pub const MAX_EVENT_ITEM_SIZE: usize = 1024 * 1024;

/// Maximum individual attachment size (100 MB).
pub const MAX_ATTACHMENT_SIZE: usize = 100 * 1024 * 1024;
