use libc::*;

use *;

pub const ENGINE_METHOD_ALL: c_uint = 0xFFFF;

extern "C" {
	#[cfg(ossl110)]
	pub fn ENGINE_by_id(id: *const c_char) -> *mut ENGINE;
	#[cfg(ossl110)]
	pub fn ENGINE_init(engine: *mut ENGINE);
	#[cfg(ossl110)]
	pub fn ENGINE_set_default(engine: *mut ENGINE, flag: c_uint);
        #[cfg(ossl110)]
 	pub fn ENGINE_finish(engine: *mut ENGINE);
	#[cfg(ossl110)]
	pub fn ENGINE_free(engine: *mut ENGINE); 
        pub fn ENGINE_load_builtin_engines();
  
}