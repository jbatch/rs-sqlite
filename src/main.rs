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
            _ => bail!(format!("Invalid PageType byte: {:x?}", byte)),
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

#[derive(Debug)]
enum ColumnType {
    NULL,
    U8,
    U16,
    U24,
    U32,
    U48,
    U64,
    F64,
    ZERO,
    ONE,
    BLOB(u64),
    TEXT(u64),
}

impl ColumnType {
    pub fn new(serial_type: u64) -> Result<ColumnType> {
        match serial_type {
            0 => Ok(ColumnType::NULL),
            1 => Ok(ColumnType::U8),
            2 => Ok(ColumnType::U16),
            3 => Ok(ColumnType::U24),
            4 => Ok(ColumnType::U32),
            5 => Ok(ColumnType::U48),
            6 => Ok(ColumnType::U64),
            7 => Ok(ColumnType::F64),
            8 => Ok(ColumnType::ZERO),
            9 => Ok(ColumnType::ONE),
            i if i > 12 && i % 2 == 0 => Ok(ColumnType::BLOB((i - 12) / 2)),
            i if i > 13 && i % 2 == 1 => Ok(ColumnType::TEXT((i - 13) / 2)),
            _ => bail!(format!("Invalid ColumnType serial_type: {}", serial_type)),
        }
    }

    pub fn len(self: &Self) -> u64 {
        match self {
            ColumnType::NULL => 0,
            ColumnType::U8 => 1,
            ColumnType::U16 => 2,
            ColumnType::U24 => 3,
            ColumnType::U32 => 4,
            ColumnType::U48 => 6,
            ColumnType::U64 => 8,
            ColumnType::F64 => 8,
            ColumnType::ZERO => 0,
            ColumnType::ONE => 0,
            ColumnType::BLOB(l) => *l,
            ColumnType::TEXT(l) => *l,
        }
    }
}

#[derive(Debug)]
pub enum ColumnValue {
    NULL,
    U8(u8),
    U16(u16),
    U24(u32),
    U32(u32),
    U48(u64),
    U64(u64),
    F64(f64),
    ZERO,
    ONE,
    BLOB(Vec<u8>),
    TEXT(String),
}

impl ColumnValue {
    fn new(bytes: &Vec<u8>, column_type: &ColumnType, index: usize) -> Result<ColumnValue> {
        let value_length = column_type.len();

        match column_type {
            ColumnType::NULL => Ok(ColumnValue::NULL),
            ColumnType::U8 => Ok(ColumnValue::NULL),
            ColumnType::U16 => Ok(ColumnValue::NULL),
            ColumnType::U24 => Ok(ColumnValue::NULL),
            ColumnType::U32 => Ok(ColumnValue::NULL),
            ColumnType::U48 => Ok(ColumnValue::NULL),
            ColumnType::U64 => Ok(ColumnValue::NULL),
            ColumnType::F64 => Ok(ColumnValue::NULL),
            ColumnType::ZERO => Ok(ColumnValue::NULL),
            ColumnType::ONE => Ok(ColumnValue::NULL),
            ColumnType::BLOB(_) => Ok(ColumnValue::NULL),
            ColumnType::TEXT(_) => {
                let end_index = index + value_length as usize;
                let text_bytes = (index..end_index)
                    .map(|v| bytes[v as usize])
                    .collect::<Vec<u8>>();
                Ok(ColumnValue::TEXT(String::from_utf8(text_bytes)?))
            }
        }
    }
}

#[derive(Debug)]
pub struct Cell {
    payload_len: u8,
    rowid: u8,
    header_length: u8,
    column_values: Vec<ColumnValue>,
    body_length: usize,
}

impl Cell {
    fn new(bytes: &Vec<u8>, cell_pointer: &u16) -> Result<Cell> {
        let mut index = *cell_pointer as usize;
        println!("creating Cell at index: {:#2x}", index);

        let payload_len = bytes[index];
        index += 1;

        let rowid = bytes[index];
        index += 1;

        let header_length = bytes[index];
        index += 1;

        let mut column_types: Vec<ColumnType> = vec![];
        // Start with read_header_bytes of one for the length byte
        let mut read_header_bytes = 1;
        // Reads a number of varints from the input bytes until we've read header_length bytes total
        while read_header_bytes < header_length {
            let mut buf: u64 = 0;
            for i in 1..9 {
                let byte = bytes[index];
                index += 1;
                read_header_bytes += 1;
                if i == 9 {
                    buf = (buf << 8) | (byte as u64);
                    break;
                }
                buf = (buf << 7) | ((byte & 0b01111111) as u64);
                if (byte & 0b10000000) >> 7 == 0 {
                    break;
                }
            }
            let column_type = ColumnType::new(buf)?;
            column_types.push(column_type);
        }
        let body_length: usize = column_types.iter().map(|t| t.len() as usize).sum();

        let mut column_values: Vec<ColumnValue> = vec![];
        for column_type in column_types {
            let column_value = ColumnValue::new(bytes, &column_type, index)?;
            index += column_type.len() as usize;
            column_values.push(column_value);
        }

        Ok(Cell {
            payload_len: payload_len,
            rowid: rowid,
            header_length: header_length,
            column_values: column_values,
            body_length: body_length,
        })
    }
}

pub struct Page {
    pub page_header: PageHeader,
    pub cell_pointers: Vec<u16>,
    pub cells: Vec<Cell>,
}

impl Page {
    pub fn new(bytes: &Vec<u8>) -> Result<Page> {
        let mut index = 100;
        let page_type = PageType::new(&bytes[index])?;
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

        println!("Cells {:?}", cells);

        Ok(Page {
            page_header: PageHeader {
                page_type: page_type,
                freeblock: freeblock,
                number_cells: number_cells,
                content_area_start: content_start_area,
                number_fragmented_bytes: number_fragmented_bytes,
            },
            cell_pointers: cell_pointers,
            cells: cells,
        })
    }
}

// pub struct Page {
//     pub page_header: PageHeader,
//     pub cell_pointers: Vec<u16>,
//     pub cells: Vec<Cell>,
// }

pub struct TableSchema {
    name: String,
    root_page: usize,
    sql: String,
}

impl TableSchema {
    pub fn new(cell: &Cell) -> Result<TableSchema> {
        println!(
            "{:?}",
            (
                cell.column_values.get(1).unwrap(),
                cell.column_values.get(3).unwrap(),
                cell.column_values.get(4).unwrap(),
            )
        );
        if let (ColumnValue::TEXT(name), ColumnValue::NULL, ColumnValue::TEXT(sql)) = (
            cell.column_values.get(1).unwrap(),
            cell.column_values.get(3).unwrap(),
            cell.column_values.get(4).unwrap(),
        ) {
            return Ok(TableSchema {
                name: name.to_string(),
                root_page: 0,
                sql: sql.to_string(),
            });
        }
        bail!("invalid cell state");
    }
}

pub struct IndexSchema {
    name: String,
    table_name: String,
    root_page: usize,
    sql: String,
}

pub struct SqliteSchema {
    tables: Vec<TableSchema>,
    indexes: Vec<IndexSchema>,
}

impl SqliteSchema {
    pub fn new(root_page: &Page) -> Result<SqliteSchema> {
        let tables: Vec<TableSchema> = root_page
            .cells
            .iter()
            .filter(|cell| {
                if let ColumnValue::TEXT(t) = cell.column_values.get(0).unwrap() {
                    t == "table"
                } else {
                    false
                }
            })
            .map(|cell| TableSchema::new(cell).unwrap())
            .collect();
        Ok(SqliteSchema {
            tables: vec![],
            indexes: vec![],
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
            let page_size: u16 = u16::from_be_bytes([header[16], header[17]]);
            println!("page_size {}", page_size);

            let mut root_bytes = vec![0u8; (page_size) as usize];
            file.read_exact_at(&mut root_bytes, 0)?;
            println!("bytes {:x?}", root_bytes);
            let root_page = Page::new(&root_bytes)?;
            let sqlite_schema = SqliteSchema::new(&root_page);

            println!("database page size: {}", page_size);
            println!("number of tables: {}", root_page.cell_pointers.len())
        }
        _ => bail!("Missing or invalid command passed: {}", command),
    }

    Ok(())
}
