const CREATE_NAME: &'static str = std::env!("CARGO_CRATE_NAME");

#[test]
pub fn crate_name() {
    //panic!("test crate name: {}", std::env!("CARGO_CRATE_NAME"));
    //panic!("test crate name: {}", std::env!("CARGO_BIN_EXE_fs_mock_entry_mock"));
    //panic!("test crate name: {}", std::env::var("CARGO_BIN_EXE_fs_mock_entry_mock").unwrap());

    //panic!("CARGO_CRATE_NAME: {}", std::env!("CARGO_CRATE_NAME")); // -> fs_mock_entry_mock
}
