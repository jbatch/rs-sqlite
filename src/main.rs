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
    pub fn new(byte: &u8) -> Result<PageType> {
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

enum SqliteSchemaRecordType {
    TABLE,
    INDEX,
    VIEW,
    TRIGGER,
}

impl SqliteSchemaRecordType {
    pub fn new(s: &str) -> Result<SqliteSchemaRecordType> {
        match s {
            "table" => Ok(SqliteSchemaRecordType::TABLE),
            "index" => Ok(SqliteSchemaRecordType::INDEX),
            "view" => Ok(SqliteSchemaRecordType::VIEW),
            "trigger" => Ok(SqliteSchemaRecordType::TRIGGER),
            _ => bail!(format!("Invalid SqliteSchemaRecordType string")),
        }
    }
}

pub struct SqliteSchemaRecord {
    record_type: SqliteSchemaRecordType,
    name: String,
    table_name: String,
    root_page: u8,
    sql: String,
}
pub struct SqliteSchemaTable {
    rows: Vec<SqliteSchemaRecord>,
}

impl SqliteSchemaTable {
    fn new(bytes: &mut IntoIter<u8>) -> Result<SqliteSchemaTable> {
        Ok(SqliteSchemaTable { rows: vec![] })
    }
}

pub struct Cell {}

impl Cell {
    fn new(bytes: &Vec<u8>, cell_pointer: &u16) -> Result<Cell> {
        Ok(Cell {})
    }
}

pub struct Page {
    pub page_header: PageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<Cell>,
}

impl Page {
    pub fn new(bytes: &Vec<u8>) -> Result<Page> {
        let mut index = 0;
        let page_type = PageType::new(bytes.get(index).unwrap())?;
        index += 1;

        let freeblock_bytes = vec![index, index + 1]
            .iter()
            .map(|v| bytes[*v])
            .collect::<Vec<u8>>();
        index += 2;
        let freeblock = u16::from_be_bytes(freeblock_bytes.try_into().unwrap());

        let number_cells_bytes = vec![index, index + 1]
            .iter()
            .map(|v| bytes[*v])
            .collect::<Vec<u8>>();
        index += 2;
        let number_cells = u16::from_be_bytes(number_cells_bytes.try_into().unwrap());

        let content_start_area_bytes = vec![index, index + 1]
            .iter()
            .map(|v| bytes[*v])
            .collect::<Vec<u8>>();
        index += 2;
        let content_start_area = u16::from_be_bytes(content_start_area_bytes.try_into().unwrap());

        let number_fragmented_bytes_bytes =
            vec![index].iter().map(|v| bytes[*v]).collect::<Vec<u8>>();
        index += 1;
        let number_fragmented_bytes =
            u8::from_be_bytes(number_fragmented_bytes_bytes.try_into().unwrap());

        let cell_pointers = (index..(index + number_cells as usize * 2))
            .map(|v| bytes[v])
            .collect::<Vec<u8>>()
            .chunks(2)
            .map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]))
            .collect::<Vec<u16>>();

        let cells: Vec<Cell> = cell_pointers
            .iter()
            .map(|cell_pointer| Cell::new(bytes, cell_pointer).unwrap())
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

            let mut root_bytes = vec![0u8; (page_size - 100) as usize];
            file.read_exact_at(&mut root_bytes, 100)?;
            // println!("root {:x?}", root);
            // println!(
            //     "cell0 {:x?} {:x?} {:x?} ... {:x?}",
            //     root[3779 - 1],
            //     root[3779],
            //     root[3779 + 1],
            //     root[3779 + 4]
            // );
            let page = Page::new(&root_bytes)?;

            println!("database page size: {}", page_size);
            println!("number of tables: {}", page.cell_pointers.len())
        }
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}
