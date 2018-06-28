// http://contain-rs.github.io/bit-vec/bit_vec/struct.BitVec.html
// https://github.com/hsutter/gcpp/blob/master/gpage.h
// https://github.com/hsutter/gcpp/blob/master/deferred_heap.h
// https://github.com/pczarn/any-arena/blob/master/src/any_arena.rs
// https://github.com/SimonSapin/rust-typed-arena/blob/master/src/lib.rs
// https://github.com/Manishearth/rust-gc/tree/master/gc
// https://doc.rust-lang.org/alloc/raw_vec/struct.RawVec.html
use bit_vec::BitVec;
use itertools::Itertools;

use std::mem;
use std::cmp::max;
use std::cell::RefCell;

const MIN_ALLOC_DEFAULT: usize = 4;

pub struct Chunk {
    data: Vec<u8>,
    min_alloc: usize,
    used: RefCell<BitVec>,
    starts_alloc: RefCell<BitVec>,
}

impl Chunk {
    pub fn with_size(size: usize) -> Chunk {
        Chunk::with_size_and_min_alloc(size, MIN_ALLOC_DEFAULT)
    }
    pub fn with_size_and_min_alloc(size: usize, min_alloc: usize) -> Chunk {
        let num_alloc_locs = size / min_alloc;
        Chunk {
            data: Vec::with_capacity(size),
            min_alloc,
            used: RefCell::new(BitVec::with_capacity(num_alloc_locs)),
            starts_alloc: RefCell::new(BitVec::with_capacity(num_alloc_locs)),
        }
    }

    pub fn is_empty(&self) -> bool {
        assert!(self.used.borrow().any() || self.starts_alloc.borrow().none());
        self.used.borrow().none()
    }

    pub fn alloc_stacklike<T>(&self) -> Result<*mut T, ()> {
        let next_byte = self.alloc_to_idx(self.used.borrow().len());

        let start_idx = round_up(next_byte, max(mem::align_of::<T>(), self.min_alloc));
        let start_loc = self.idx_to_alloc(start_idx);
        // let loc_align = 1 + (mem::align_of::<T>() - 1) / self.min_alloc;

        let bytes_needed = max(mem::size_of::<T>(), self.min_alloc);
        let locs_needed = 1 + (bytes_needed - 1) / self.min_alloc;

        if start_idx + bytes_needed >= self.data.capacity() {
            // Not enough room for the allocation
            return Err(())
        }
        if start_loc + locs_needed >= self.num_alloc_locs() {
            // Another way of checking for the previous problem
            return Err(())
        }

        let padding_locs = start_loc - next_byte;
        {
            let mut used = self.used.borrow_mut();
            let mut starts_alloc = self.starts_alloc.borrow_mut();

            used.grow(padding_locs, false);
            starts_alloc.grow(padding_locs, false);

            used.grow(locs_needed, true);
            starts_alloc.push(true);
            starts_alloc.grow(locs_needed - 1, false);
        }

        let mut ptr = self.data.as_ptr();
        ptr = unsafe { ptr.offset(start_idx as isize) };
        Ok(ptr as *mut T)
    }

    pub fn alloc<T>(&self) -> Result<*mut T, ()> {
        // let next_byte = self.alloc_to_idx(self.used.borrow().len());
        //
        // let start_idx = round_up(next_byte, max(mem::align_of::<T>(), self.min_alloc));
        // let start_loc = self.idx_to_alloc(start_idx);
        // let loc_align = 1 + (mem::align_of::<T>() - 1) / self.min_alloc;

        let bytes_needed = mem::size_of::<T>();
        let locs_needed = 1 + (bytes_needed - 1) / self.min_alloc;

        let locs_per_step = 1 + (mem::align_of::<T>() - 1) / self.min_alloc;

        println!("searching for storage loc");
        let mut storage_loc_step = 0;
        for contig_usage in &self.used.borrow().iter()
            .chunks(locs_per_step) {
                storage_loc_step += 1;
                if contig_usage.take(locs_needed).all(|b| b) {
                    storage_loc_step -= 1;
                    break;
                }
        };


        let storage_loc = storage_loc_step * locs_per_step;

        if storage_loc + locs_needed >= self.num_alloc_locs() {
            // Not enough room for the allocation
            return Err(())
        }

        if storage_loc + locs_needed > self.used.borrow().len() {
            let mut used = self.used.borrow_mut();
            let mut starts_alloc = self.starts_alloc.borrow_mut();
            //
            // if used.len() < storage_loc {
            //     let padding_locs = storage_loc - used.len();
            //     used.grow(padding_locs, false);
            //     starts_alloc.grow(padding_locs, false);
            // }
            //
            // let locs_to_add = storage_loc + locs_needed - used.len();
            // used.grow(locs_to_add, true);
            // starts_alloc.push(true);
            // starts_alloc.grow(locs_to_add - 1, false);

            let locs_to_add = storage_loc + locs_needed - used.len();
            used.grow(locs_to_add, false);
            starts_alloc.grow(locs_to_add, false);
        }

        {
            let mut used = self.used.borrow_mut();
            let mut starts_alloc = self.starts_alloc.borrow_mut();

            starts_alloc.set(storage_loc, true);

            for i in storage_loc..(storage_loc + locs_needed) {
                used.set(i, true);
            }
        }

        let ptr = self.data.as_ptr();
        Ok(unsafe { ptr.add(self.alloc_to_idx(storage_loc)) } as *mut T)
    }

    pub fn dealloc<T>(&self, ptr: *const T) {
        assert!(!ptr.is_null());
        assert!(self.contains(ptr));

        let data_idx = ptr as usize - self.data.as_ptr() as usize;
        let data_loc = self.idx_to_alloc(data_idx);

        let mut used = self.used.borrow_mut();
        let mut starts_alloc = self.starts_alloc.borrow_mut();

        starts_alloc.set(data_loc, false);

        let mut next_start_loc = data_loc;
        for start_val in starts_alloc.iter().skip(data_loc) {
            next_start_loc += 1;
            if start_val {
                break;
            }
        }

        for i in data_loc..next_start_loc {
            // Stop setting once we hit unused locs.
            if !used.get(i).unwrap() {
                break;
            }
            used.set(i, false);
        }
    }

    pub fn contains<T>(&self, ptr: *const T) -> bool {
        let data_start = self.data.as_ptr();
        let data_end = unsafe { data_start.offset(self.data.capacity() as isize) } as usize;
        let data_start = data_start as usize;
        let ptr_val = ptr as usize;

        ptr_val >= data_start && ptr_val < data_end
    }

    fn num_alloc_locs(&self) -> usize {
        self.used.borrow().capacity()
    }
    fn alloc_to_idx(&self, alloc_loc: usize) -> usize {
        alloc_loc * self.min_alloc
    }

    fn idx_to_alloc(&self, buf_idx: usize) -> usize {
        buf_idx / self.min_alloc
    }
}

// Taken from any_arena crate
#[inline]
fn round_up(base: usize, align: usize) -> usize {
    base.checked_add(align - 1).unwrap() & !(align - 1)
}

#[cfg(test)]
mod tests {
    use page_size;
    use super::*;

    const CHUNK_SIZE: usize = 1024;
    const MIN_ALLOC: usize = 4;

    fn new_chunk() -> Chunk {
        Chunk::with_size_and_min_alloc(CHUNK_SIZE, MIN_ALLOC)
    }

    #[test]
    fn can_be_created() {
        Chunk::with_size(page_size::get());
    }

    #[test]
    fn knows_when_empty() {
        let chunk = new_chunk();
        assert!(chunk.is_empty());
        chunk.used.borrow_mut().push(false);
        assert!(chunk.is_empty());
        chunk.used.borrow_mut().push(true);
        assert!(!chunk.is_empty());
    }

    #[test]
    fn is_used_after_alloc() {
        let chunk = new_chunk();
        assert!(chunk.is_empty());
        chunk.alloc::<Chunk>().unwrap();
        assert!(!chunk.is_empty());
    }

    #[test]
    fn knows_what_addrs_it_contins() {
        let chunk_a = new_chunk();
        let in_a = chunk_a.alloc::<Chunk>().unwrap();
        assert!(chunk_a.contains(in_a));

        let chunk_b = new_chunk();
        let in_b = chunk_b.alloc::<[u8; 5]>().unwrap();
        assert!(chunk_b.contains(in_b));

        assert!(!chunk_a.contains(in_b));
        assert!(!chunk_b.contains(in_a));

        let data_a = chunk_a.data.as_ptr();
        let before_a = unsafe { data_a.offset(-1) };
        assert!(!chunk_a.contains(before_a));
        let after_a = unsafe { data_a.offset(chunk_a.data.capacity() as isize) };
        assert!(!chunk_a.contains(after_a));
    }

    #[test]
    fn is_empty_after_deallocating() {
        let chunk = new_chunk();
        let ptr = chunk.alloc::<Chunk>().unwrap();
        assert!(!chunk.is_empty());
        chunk.dealloc(ptr);
        assert!(chunk.is_empty());
    }
}