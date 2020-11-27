//! Represents arbitrary bytes type
//! 
//! ```
//! use no_proto::error::NP_Error;
//! use no_proto::NP_Factory;
//! use no_proto::pointer::bytes::NP_Bytes;
//! use no_proto::here;
//! 
//! let factory: NP_Factory = NP_Factory::new(r#"{
//!    "type": "bytes"
//! }"#)?;
//!
//! let mut new_buffer = factory.empty_buffer(None, None);
//! new_buffer.set(here(), NP_Bytes::new([0, 1, 2, 3, 4].to_vec()))?;
//! 
//! assert_eq!(Box::new(NP_Bytes::new([0, 1, 2, 3, 4].to_vec())), new_buffer.get::<NP_Bytes>(here())?.unwrap());
//!
//! # Ok::<(), NP_Error>(()) 
//! ```
//! 

use crate::{json_flex::JSMAP, schema::{NP_Parsed_Schema}};
use crate::schema::NP_Schema;
use crate::error::NP_Error;
use crate::memory::{NP_Size};
use crate::{schema::{NP_TypeKeys}, pointer::NP_Value, json_flex::NP_JSON};
use core::hint::unreachable_unchecked;

use alloc::vec::Vec;
use alloc::string::String;
use alloc::boxed::Box;
use alloc::{borrow::ToOwned};
use super::{NP_Cursor_Addr};
use crate::NP_Memory;

/// Holds arbitrary byte data.
/// 
/// Check out documentation [here](../bytes/index.html).
/// 
#[derive(Debug, Eq, PartialEq)]
pub struct NP_Bytes<'bytes> {
    /// The bytes of the vec in this type
    pub bytes: &'bytes [u8]
}

impl<'bytes> NP_Bytes<'bytes> {
    /// Create a new bytes type with the provided Vec
    pub fn new(bytes: &'bytes [u8]) -> Self {
        NP_Bytes { bytes: bytes }
    }
}



impl<'bytes> Default for NP_Bytes<'bytes> {
    fn default() -> Self { 
        NP_Bytes { bytes: &[] }
     }
}

impl<'value> NP_Value<'value> for NP_Bytes<'value> {


    fn type_idx() -> (&'value str, NP_TypeKeys) { ("bytes", NP_TypeKeys::Bytes) }
    fn self_type_idx(&self) -> (&'value str, NP_TypeKeys) { ("bytes", NP_TypeKeys::Bytes) }

    fn schema_to_json(schema: &Vec<NP_Parsed_Schema<'value>>, address: usize)-> Result<NP_JSON, NP_Error> {
        let mut schema_json = JSMAP::new();
        schema_json.insert("type".to_owned(), NP_JSON::String(Self::type_idx().0.to_string()));

        match &schema[address] {
            NP_Parsed_Schema::Bytes { i: _, sortable: _, default, size} => {
                if *size > 0 {
                    schema_json.insert("size".to_owned(), NP_JSON::Integer(*size as i64));
                }
        
                if let Some(d) = default {
                    let default_bytes: Vec<NP_JSON> = d.iter().map(|value| {
                        NP_JSON::Integer(i64::from(*value))
                    }).collect();
                    schema_json.insert("default".to_owned(), NP_JSON::Array(default_bytes));
                }
            },
            _ => { unsafe { unreachable_unchecked() } }
        }


        Ok(NP_JSON::Dictionary(schema_json))
    }

    fn schema_default(schema: &'value NP_Parsed_Schema) -> Option<Box<Self>> {

        match schema {
            NP_Parsed_Schema::Bytes { i: _, sortable: _, default, size: _} => {
                if let Some(d) = default {
                    Some(Box::new(NP_Bytes { bytes: *d.clone() }))
                } else {
                    None
                }
            },
            _ => { unsafe { unreachable_unchecked() } }
        }
    }

    fn set_value(cursor_addr: NP_Cursor_Addr, memory: NP_Memory, value: &Self) -> Result<NP_Cursor_Addr, NP_Error> {
 
        let bytes = value.bytes;

        let str_size = bytes.len() as usize;
    
        let cursor = cursor_addr.get_data(&memory).unwrap();

        if cursor_addr.is_virtual { panic!() }
    
        let write_bytes = memory.write_bytes();

        let size = match &**cursor.schema {
            NP_Parsed_Schema::Bytes { i: _, sortable: _, default: _, size} => {
                size
            },
            _ => { panic!() }
        };

        if *size > 0 { // fixed size bytes

            if cursor.address_value == 0 { // malloc new bytes

                let mut empty_bytes: Vec<u8> = Vec::with_capacity(*size as usize);
                for _x in 0..(*size as usize) {
                    empty_bytes.push(0);
                }
                
                cursor.address_value = memory.malloc(empty_bytes)? as usize;
                memory.set_value_address(cursor.address, cursor.address_value);
            }

            for x in 0..(*size as usize) {
                if x < bytes.len() { // assign values of bytes
                    write_bytes[(cursor.address_value + x)] = bytes[x];
                } else { // rest is zeros
                    write_bytes[(cursor.address_value + x)] = 0;
                }
            }
    
            return Ok(cursor_addr)
        }

        // flexible size

        let prev_size: usize = if cursor.address_value != 0 {
            match memory.size {
                NP_Size::U8 => {
                    let size_bytes: u8 = memory.get_1_byte(cursor.address_value).unwrap_or(0);
                    u8::from_be_bytes([size_bytes]) as usize
                },
                NP_Size::U16 => {
                    let size_bytes: &[u8; 2] = memory.get_2_bytes(cursor.address_value).unwrap_or(&[0; 2]);
                    u16::from_be_bytes(*size_bytes) as usize
                },
                NP_Size::U32 => { 
                    let size_bytes: &[u8; 4] = memory.get_4_bytes(cursor.address_value).unwrap_or(&[0; 4]);
                    u32::from_be_bytes(*size_bytes) as usize
                }
            }
        } else {
            0 as usize
        };

        if prev_size >= str_size as usize { // previous string is larger than this one, use existing memory
        
            // update bytes length in buffer
            match memory.size {
                NP_Size::U8 => { 
                    if str_size > core::u8::MAX as usize {
                        return Err(NP_Error::new("String too large!"))
                    }
                    let size_bytes = (str_size as u8).to_be_bytes();
                    // set string size
                    write_bytes[cursor.address_value] = size_bytes[0];
                }
                NP_Size::U16 => { 
                    if str_size > core::u16::MAX as usize {
                        return Err(NP_Error::new("String too large!"))
                    }
                    let size_bytes = (str_size as u16).to_be_bytes();
                    // set string size
                    for x in 0..size_bytes.len() {
                        write_bytes[(cursor.address_value + x)] = size_bytes[x];
                    }
                },
                NP_Size::U32 => { 
                    if str_size > core::u32::MAX as usize {
                        return Err(NP_Error::new("String too large!"))
                    }
                    let size_bytes = (str_size as u32).to_be_bytes();
                    // set string size
                    for x in 0..size_bytes.len() {
                        write_bytes[(cursor.address_value + x)] = size_bytes[x];
                    }
                }
            };


            let offset = memory.addr_size_bytes();

            // set bytes
            for x in 0..bytes.len() {
                write_bytes[(cursor.address_value + x + offset) as usize] = bytes[x as usize];
            }
    
            return Ok(cursor_addr)
        } else { // not enough space or space has not been allocted yet
            
            // first bytes are string length
            cursor.address_value = match memory.size {
                NP_Size::U8 => { 
                    if str_size > core::u8::MAX as usize {
                        return Err(NP_Error::new("String too large!"))
                    }
                    let size_bytes = (str_size as u8).to_be_bytes();
                    memory.malloc_borrow(&size_bytes)?
                },
                NP_Size::U16 => { 
                    if str_size > core::u16::MAX as usize {
                        return Err(NP_Error::new("String too large!"))
                    }
                    let size_bytes = (str_size as u16).to_be_bytes();
                    memory.malloc_borrow(&size_bytes)?
                },
                NP_Size::U32 => { 
                    if str_size > core::u32::MAX as usize {
                        return Err(NP_Error::new("String too large!"))
                    }
                    let size_bytes = (str_size as u32).to_be_bytes();
                    memory.malloc_borrow(&size_bytes)?
                }
            };

            memory.set_value_address(cursor.address, cursor.address_value);

            memory.malloc_borrow(bytes)?;

            return Ok(cursor_addr);
        }
            
    }
    

    fn into_value<'into>(cursor_addr: NP_Cursor_Addr, memory: &'value NP_Memory) -> Result<Option<Box<Self>>, NP_Error> {
        let cursor = cursor_addr.get_data(&memory).unwrap();

        // empty value
        if cursor.address_value == 0 {
            return Ok(None);
        }

        match &**cursor.schema {
            NP_Parsed_Schema::Bytes { i: _, sortable: _, default: _, size} => {
                if *size > 0 { // fixed size
            
                    let size = *size as usize;
                    
                    // get bytes
                    let bytes = &memory.read_bytes()[(cursor.address_value)..(cursor.address_value+size)];
        
                    return Ok(Some(Box::new(NP_Bytes { bytes: bytes})))
        
                } else { // dynamic size
                    // get size of bytes
        
                    let bytes_size: usize = match memory.size {
                        NP_Size::U8 => {
                            let mut size_bytes: [u8; 1] = [0; 1];
                            size_bytes.copy_from_slice(&memory.read_bytes()[cursor.address_value..(cursor.address_value+1)]);
                            u8::from_be_bytes(size_bytes) as usize
                        },
                        NP_Size::U16 => {
                            let mut size_bytes: [u8; 2] = [0; 2];
                            size_bytes.copy_from_slice(&memory.read_bytes()[cursor.address_value..(cursor.address_value+2)]);
                            u16::from_be_bytes(size_bytes) as usize
                        },
                        NP_Size::U32 => { 
                            let mut size_bytes: [u8; 4] = [0; 4];
                            size_bytes.copy_from_slice(&memory.read_bytes()[cursor.address_value..(cursor.address_value+4)]);
                            u32::from_be_bytes(size_bytes) as usize
                        }
                    };
        
                    // get bytes
                    let bytes = match memory.size {
                        NP_Size::U8 => { &memory.read_bytes()[(cursor.address_value+1)..(cursor.address_value+1+bytes_size)] },
                        NP_Size::U16 => { &memory.read_bytes()[(cursor.address_value+2)..(cursor.address_value+2+bytes_size)] },
                        NP_Size::U32 => { &memory.read_bytes()[(cursor.address_value+4)..(cursor.address_value+4+bytes_size)] }
                    };
        
                    return Ok(Some(Box::new(NP_Bytes { bytes: bytes})))
                }
            },
            _ => { unsafe { unreachable_unchecked() } }
        }
    }

    fn to_json(cursor_addr: NP_Cursor_Addr, memory: &'value NP_Memory) -> NP_JSON {


        match Self::into_value(cursor_addr, memory) {
            Ok(x) => {
                match x {
                    Some(y) => {

                        let bytes = y.bytes.into_iter().map(|x| NP_JSON::Integer(*x as i64)).collect();

                        NP_JSON::Array(bytes)
                    },
                    None => {
                        let cursor = cursor_addr.get_data(&memory).unwrap();
                        match &**cursor.schema {
                            NP_Parsed_Schema::Bytes { i: _, size: _, default, sortable: _ } => {
                                match default {
                                    Some(x) => {
                                        let bytes = x.iter().map(|v| {
                                            NP_JSON::Integer(*v as i64)
                                        }).collect::<Vec<NP_JSON>>();

                                        NP_JSON::Array(bytes)
                                    },
                                    None => NP_JSON::Null
                                }
                            },
                            _ => { unsafe { unreachable_unchecked() } }
                        }
                    }
                }
            },
            Err(_e) => {
                NP_JSON::Null
            }
        }
    }

    fn get_size(cursor_addr: NP_Cursor_Addr, memory: NP_Memory) -> Result<usize, NP_Error> {
        let cursor = cursor_addr.get_data(&memory).unwrap();

        // empty value
        if cursor.address_value == 0 {
            return Ok(0)
        }
        
    
        match &**cursor.schema {
            NP_Parsed_Schema::Bytes { i: _, size, default: _, sortable: _ } => {
                // fixed size
                if *size > 0 { 
                    return Ok(*size as usize)
                }
    
                // dynamic size
                let bytes_size = memory.read_address(cursor.address_value) + memory.addr_size_bytes();
                
                // return total size of this string
                return Ok(bytes_size);
            },
            _ => { unsafe { unreachable_unchecked() } }
        }

    }

    fn from_json_to_schema(schema: Vec<NP_Parsed_Schema<'value>>, json_schema: &'value NP_JSON) -> Result<Option<(Vec<u8>, Vec<NP_Parsed_Schema<'value>>)>, NP_Error> {

        let type_str = NP_Schema::_get_type(json_schema)?;

        if "bytes" == type_str || "u8[]" == type_str || "[u8]" == type_str {

            let mut has_fixed_size = false;
            let mut schema_data: Vec<u8> = Vec::new();
            schema_data.push(NP_TypeKeys::Bytes as u8);

            let size = match json_schema["size"] {
                NP_JSON::Integer(x) => {
                    has_fixed_size = true;
                    if x < 1 {
                        return Err(NP_Error::new("Fixed size for bytes must be larger than 1!"));
                    }
                    if x > u16::MAX.into() {
                        return Err(NP_Error::new("Fixed size for bytes cannot be larger than 2^16!"));
                    }
                    schema_data.extend((x as u16).to_be_bytes().to_vec());
                    x as u16
                },
                NP_JSON::Float(x) => {
                    has_fixed_size = true;
                    if x < 1.0 {
                        return Err(NP_Error::new("Fixed size for bytes must be larger than 1!"));
                    }
                    if x > u16::MAX.into() {
                        return Err(NP_Error::new("Fixed size for bytes cannot be larger than 2^16!"));
                    }

                    schema_data.extend((x as u16).to_be_bytes().to_vec());
                    x as u16
                },
                _ => {
                    schema_data.extend(0u16.to_be_bytes().to_vec());
                    0
                }
            };

            let default = match &json_schema["default"] {
                NP_JSON::Array(bytes) => {

                    let default_bytes: Vec<u8> = bytes.into_iter().map(|v| {
                        match v {
                            NP_JSON::Integer(x) => { *x as u8},
                            _ => { 0u8 }
                        }
                    }).collect();
                    let length = default_bytes.len() as u16 + 1;
                    schema_data.extend(length.to_be_bytes().to_vec());
                    schema_data.extend(default_bytes.clone());
                    Some(Box::new(default_bytes))
                },
                _ => {
                    schema_data.extend(0u16.to_be_bytes().to_vec());
                    None
                }
            };

            schema.push(NP_Parsed_Schema::Bytes {
                i: NP_TypeKeys::Bytes,
                size: size,
                default: if let Some(d) = default {
                    Some(Box::new(&d[..]))
                } else {
                    None
                },
                sortable: has_fixed_size
            });

            return Ok(Some((schema_data, schema)));
        }
        
        Ok(None)
    }

    fn from_bytes_to_schema(schema: Vec<NP_Parsed_Schema<'value>>, address: usize, bytes: &'value Vec<u8>) -> Vec<NP_Parsed_Schema<'value>> {
        // fixed size
        let fixed_size = u16::from_be_bytes([
            bytes[address + 1],
            bytes[address + 2]
        ]);

        // default value size
        let default_size = u16::from_be_bytes([
            bytes[address + 3],
            bytes[address + 4]
        ]) as usize;

        if default_size == 0 {
            schema.push(NP_Parsed_Schema::Bytes {
                i: NP_TypeKeys::Bytes,
                default: None,
                sortable: fixed_size > 0,
                size: fixed_size
            });
        } else {
            let default_bytes = &bytes[(address + 5)..(address + 5 + (default_size - 1))];

            schema.push(NP_Parsed_Schema::Bytes {
                i: NP_TypeKeys::Bytes,
                default: Some(Box::new(default_bytes)),
                size: fixed_size,
                sortable: fixed_size > 0
            });    
        }

        schema

    }
}

#[test]
fn schema_parsing_works() -> Result<(), NP_Error> {
    let schema = "{\"type\":\"bytes\",\"default\":[22,208,10,78,1,19,85]}";
    let factory = crate::NP_Factory::new(schema)?;
    assert_eq!(schema, factory.schema.to_json()?.stringify());

    let schema = "{\"type\":\"bytes\",\"size\":10}";
    let factory = crate::NP_Factory::new(schema)?;
    assert_eq!(schema, factory.schema.to_json()?.stringify());

    let schema = "{\"type\":\"bytes\"}";
    let factory = crate::NP_Factory::new(schema)?;
    assert_eq!(schema, factory.schema.to_json()?.stringify());
    
    Ok(())
}


#[test]
fn default_value_works() -> Result<(), NP_Error> {
    let schema = "{\"type\":\"bytes\",\"default\":[1,2,3,4]}";
    let factory = crate::NP_Factory::new(schema)?;
    let mut buffer = factory.empty_buffer(None, None);
    assert_eq!(buffer.get::<NP_Bytes>(&[])?.unwrap(), Box::new(NP_Bytes::new(&[1,2,3,4])));

    Ok(())
}

#[test]
fn fixed_size_works() -> Result<(), NP_Error> {
    let schema = "{\"type\":\"bytes\",\"size\": 20}";
    let factory = crate::NP_Factory::new(schema)?;
    let mut buffer = factory.empty_buffer(None, None);
    buffer.set(&[], NP_Bytes::new(&[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22]))?;
    assert_eq!(buffer.get::<NP_Bytes>(&[])?.unwrap(), Box::new(NP_Bytes::new(&[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20])));

    Ok(())
}

#[test]
fn set_clear_value_and_compaction_works() -> Result<(), NP_Error> {
    let schema = "{\"type\":\"bytes\"}";
    let factory = crate::NP_Factory::new(schema)?;
    let mut buffer = factory.empty_buffer(None, None);
    buffer.set(&[], NP_Bytes::new(&[1,2,3,4,5,6,7,8,9,10,11,12,13]))?;
    assert_eq!(buffer.get(&[])?.unwrap(), Box::new(NP_Bytes::new(&[1,2,3,4,5,6,7,8,9,10,11,12,13])));
    buffer.del(&[])?;
    assert_eq!(buffer.get::<NP_Bytes>(&[])?, None);

    buffer.compact(None, None)?;
    assert_eq!(buffer.calc_bytes()?.current_buffer, 4usize);

    Ok(())
}