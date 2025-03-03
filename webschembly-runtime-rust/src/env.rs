unsafe extern "C" {
    pub fn js_instantiate(buf_ptr: i32, buf_size: i32);
    pub fn js_write_buf(fd: i32, buf_ptr: i32, buf_len: i32);
    pub fn js_webschembly_log(buf_ptr: i32, buf_len: i32);
}
