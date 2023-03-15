use anyhow::{bail, Result};
use std::fs::File;
use std::io::{prelude::*, BufReader};
use std::os::unix::prelude::FileExt;
use std::vec::IntoIter;

#[derive(Debug)]
pub enum PageType {
    TableInteriorPage,
    TableLeafPage,
    IndexInteriorPage,
    IndexLeafPage,
}

impl PageType {
    pub fn new(byte: u8) -> Result<PageType> {
        match byte {
            0x0d => Ok(PageType::TableLeafPage),
            0x0a => Ok(PageType::IndexLeafPage),
            0x02 => Ok(PageType::IndexInteriorPage),
            0x05 => Ok(PageType::TableInteriorPage),
            _ => bail!(format!("Invalid PageType byte")),
        }
    }
}
pub struct PageHeader {
    pub page_type: PageType,
    pub freeblock: u16,
    pub number_cells: u16,
    pub content_area_start: u16,
    pub number_fragmented_bytes: u8,
}

pub struct Cell {}

impl Cell {}

pub struct Page {
    pub page_header: PageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<Cell>,
}

impl Page {
    pub fn new(bytes: &mut IntoIter<u8>) -> Result<Page> {
        let page_type = PageType::new(bytes.next().unwrap())?;
        let freeblock = u16::from_be_bytes(bytes.take(2).collect::<Vec<u8>>().try_into().unwrap());
        let number_cells =
            u16::from_be_bytes(bytes.take(2).collect::<Vec<u8>>().try_into().unwrap());
        let content_start_area =
            u16::from_be_bytes(bytes.take(2).collect::<Vec<u8>>().try_into().unwrap());
        let number_fragmented_bytes =
            u8::from_be_bytes(bytes.take(1).collect::<Vec<u8>>().try_into().unwrap());
        let cell_pointers: Vec<u16> = bytes
            .take(number_cells as usize * 2)
            .collect::<Vec<u8>>()
            .chunks(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect();
        Ok(Page {
            page_header: PageHeader {
                page_type: page_type,
                freeblock: freeblock,
                number_cells: number_cells,
                content_area_start: content_start_area,
                number_fragmented_bytes: number_fragmented_bytes,
            },
            cell_pointers: cell_pointers,
            cells: vec![],
        })
    }
}

fn main() -> Result<()> {
    // Parse arguments
    let args = std::env::args().collect::<Vec<_>>();
    match args.len() {
        0 | 1 => bail!("Missing <database path> and <command>"),
        2 => bail!("Missing <command>"),
        _ => {}
    }

    // Parse command and act accordingly
    let command = &args[2];
    match command.as_str() {
        ".dbinfo" => {
            let mut file = File::open(&args[1])?;
            let mut header = [0; 100];
            file.read_exact(&mut header)?;

            // The page size is stored at the 16th byte offset, using 2 bytes in big-endian order
            #[allow(unused_variables)]
            let page_size: u16 = u16::from_be_bytes([header[16], header[17]]);

            let mut root_bytes = vec![0u8; page_size as usize];
            file.read_exact_at(&mut root_bytes, 100)?;
            // println!("root {:x?}", root);
            // println!(
            //     "cell0 {:x?} {:x?} {:x?} ... {:x?}",
            //     root[3779 - 1],
            //     root[3779],
            //     root[3779 + 1],
            //     root[3779 + 4]
            // );
            let page = Page::new(&mut root_bytes.into_iter())?;

            println!("database page size: {}", page_size);
            println!("number of tables: {}", page.cell_pointers.len())
        }
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}
