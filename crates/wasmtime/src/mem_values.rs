use serde::{Deserialize, Serialize};
use crate::{values::Val, ValType};
use wasmtime_param_types::*;

///Buffer
///
/// The value and storage buffer type parameters or return values
#[derive(Serialize, Deserialize, Debug)]

pub struct Buffer{
    /// the index of the ptr type parameter or return
    ptr_idx: Idx,
    /// the index of the len type parameter or return
    len_idx: Idx,
    /// the underlying data, it can be empty for in-buffers before calling the function
    data: Option<Vec<u8>>
}

impl Buffer {
    /// Create Buffer with underlying data
    pub fn new(ptr_idx: Idx, len_idx: Idx, buffer: Vec<u8>) -> Self {
        Buffer { ptr_idx, len_idx, data: Some(buffer)}
    }

    /// Create Buffer without underlying data 
    pub fn new_empty(ptr_idx: Idx, len_idx: Idx) -> Self {
        Buffer{ptr_idx, len_idx, data: None}
    }
    
    /// Create a Buffer new from existing Buffer
    pub fn from_src(src_buf: &Buffer) -> Self{
        if let Some(src_data) = &src_buf.data{
            let src_data = src_data.as_slice();
            Buffer{ptr_idx: src_buf.ptr_idx, len_idx: src_buf.len_idx, data: Some(src_data.to_vec())}
        }
        else{
            Buffer{ptr_idx: src_buf.ptr_idx, len_idx: src_buf.len_idx, data: None}
        }
           
    }
}

///UnVal
///
/// Used to store unassociated parameter or return values
#[derive(Serialize, Deserialize, Debug)]
pub struct UnVal(Val);

impl From<Val> for UnVal {
    fn from(v: Val) -> Self {
        UnVal(v)
    }
}

/// ParamRetData
/// 
/// Used to pass a function's parameter value and underlying data
#[derive(Serialize, Deserialize, Debug)]
#[serde(deny_unknown_fields)]
#[serde(rename = "func_param_ret_data")]
pub struct ParamRetData{

    /// A vector of function's parameter value, specified in the same order as the function
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "Vec::new")]
    pub param_vals: Vec<Val>,

    /// A vector of underlying data "described" by the above parameter values
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "Vec::new")]
    pub param_bufs: Vec<Buffer>,

    /// A vector of function's return value, specified in the same order as the function
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "Vec::new")]
    pub ret_vals: Vec<Val>,

    /// A vector of underlying data "described" by the above return values
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[serde(default = "Vec::new")]
    pub ret_bufs: Vec<Buffer>,

    /// The function's name that the data associated with, 
    /// used to by remote to find corresponding FuncParamRetTypes to interpret the data 
    #[serde(skip_serializing_if = "String::is_empty")]
    #[serde(default = "String::new")]
    pub func_name: String,
}

impl ParamRetData {
    
    /// create ParamRetData 
    pub fn new() -> Self {
        ParamRetData { param_vals: Vec::new(), param_bufs: Vec::new(), ret_vals: Vec::new(), ret_bufs: Vec::new(), func_name: String::new()}
    }

    /// create ParamRetData with name of the function 
    pub fn new_with_name(name: &str) -> Self {
        ParamRetData { param_vals: Vec::new(), param_bufs: Vec::new(), ret_vals: Vec::new(), ret_bufs: Vec::new(), func_name: name.to_string()}
    }

    /// push parameter value into the corresponding vector
    pub fn push_param(&mut self, v: Val){
        self.param_vals.push(v)
    }

    /// push an parameter buffer into the corresponding vector
    pub fn push_param_buffer(&mut self, v: Buffer){
        self.param_bufs.push(v)
    }

    /// push unassociated return values into the corresponding vector
    pub fn push_ret(&mut self, v: Val){
        self.ret_vals.push(v)
    } 

    /// push an buffer (for return values) into the corresponding vector
    pub fn push_ret_buffer(&mut self, v: Buffer){
        self.ret_bufs.push(v)
    }

    /// get function name associated with the data
    pub fn get_func_name(&self) -> &str{
        self.func_name.as_str()
    }

    /// get the underlying data described a pair of pointer and length type parameters
    pub fn get_data_from_ptr_and_len(&mut self, ptr: i32, len: i32) -> Option<&mut [u8]>{
        for param_buf in &mut self.param_bufs {
            let ptr_idx: usize = param_buf.ptr_idx.into();
            let len_idx: usize = param_buf.len_idx.into();
            let expected_ptr = &self.param_vals[ptr_idx];
            let expected_len = &self.param_vals[len_idx];
            assert_eq!(expected_ptr.ty(), ValType::I32);
            assert_eq!(expected_len.ty(), ValType::I32);
            let expected_ptr_i32 = expected_ptr.unwrap_i32();

            let expected_len_i32 = expected_len.unwrap_i32();

            if ptr != expected_ptr_i32 || len != expected_len_i32 {
                continue;
            }

            // now we found the underlying data
            return Some(param_buf.data.as_deref_mut().unwrap());
        }
        None
    }

    /// set data for a in-buffer
    pub fn set_data_for_ptr_and_len(&mut self, ptr: i32, len: i32, src_data: &[u8]){
        for param_buf in &mut self.param_bufs {
            let ptr_idx: usize = param_buf.ptr_idx.into();
            let len_idx: usize = param_buf.len_idx.into();
            let expected_ptr = &self.param_vals[ptr_idx];
            let expected_len = &self.param_vals[len_idx];
            assert_eq!(expected_ptr.ty(), ValType::I32);
            assert_eq!(expected_len.ty(), ValType::I32);
            let expected_ptr_i32 = expected_ptr.unwrap_i32();

            let expected_len_i32 = expected_len.unwrap_i32();

            if ptr != expected_ptr_i32 || len != expected_len_i32 {
                continue;
            }
            match param_buf.data {
                Some(_) => {
                    let data_vec = param_buf.data.as_mut().unwrap();
                    data_vec.extend_from_slice(src_data);
                }
                None => {
                    let mut data_vec = Vec::new();
                    data_vec.extend_from_slice(src_data);
                    param_buf.data = Some(data_vec);
                }
            }
        }
    }

    /// serialize the current structure into JSON
    pub fn serialize_to_json(&self) -> String{
        serde_json::to_string(self).unwrap()
    }

    /// unpack self to a destination storage
    pub fn unpack_to(&self, dst: &mut ParamRetData) {
        
        dst.param_vals.clear();
        for val in &self.param_vals {
            dst.param_vals.push(val.clone())
        }

        dst.param_bufs.clear();
        for param_buf in &self.param_bufs {
            dst.param_bufs.push(Buffer::from_src(param_buf))
        }

        dst.ret_vals.clear();
        for val in &self.ret_vals{
            dst.ret_vals.push(val.clone());
        }

        dst.ret_bufs.clear();
        for ret_buf in &self.ret_bufs {
            dst.ret_bufs.push(Buffer::from_src(ret_buf));
        }
    }


    /// update the storage with function's return value (after host call's the function)
    pub fn push_ret_vals(&mut self, ret_vals: &[Val]){
        for ret in ret_vals{
            self.ret_vals.push(ret.clone());
        }
    }


}
