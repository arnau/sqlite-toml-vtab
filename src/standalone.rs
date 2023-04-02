use crate::reader::read_data;
use crate::types::{Headers, Records};
use sqlite_loadable::table::{IndexInfo, VTab, VTabArguments, VTabCursor};
use sqlite_loadable::vtab_argparse::{parse_argument, Argument, ConfigOptionValue};
use sqlite_loadable::{api, define_virtual_table};
use sqlite_loadable::{define_scalar_function, prelude::*};
use sqlite_loadable::{BestIndexError, Error, Result};
use std::path::Path;
use std::{mem, os::raw::c_int};

#[sqlite_entrypoint]
pub fn sqlite3_tomlvtab_init(db: *mut sqlite3) -> Result<()> {
    define_virtual_table::<TomlTable>(db, "toml", None)?;
    define_scalar_function(
        db,
        "toml_version",
        0,
        toml_version,
        FunctionFlags::DETERMINISTIC,
    )?;

    Ok(())
}

fn toml_version(context: *mut sqlite3_context, _values: &[*mut sqlite3_value]) -> Result<()> {
    let blurb = format!("v{}", env!("CARGO_PKG_VERSION"));
    api::result_text(context, blurb)?;

    Ok(())
}

#[repr(C)]
pub struct TomlTable {
    /// must be first
    base: sqlite3_vtab,
    db: *mut sqlite3,
    path: String,
}

impl<'vtab> VTab<'vtab> for TomlTable {
    type Aux = u8;
    type Cursor = TomlCursor;

    fn create(
        db: *mut sqlite3,
        aux: Option<&Self::Aux>,
        args: VTabArguments,
    ) -> Result<(String, Self)> {
        Self::connect(db, aux, args)
    }

    fn connect(
        db: *mut sqlite3,
        _aux: Option<&Self::Aux>,
        args: VTabArguments,
    ) -> Result<(String, Self)> {
        let arguments = parse_arguments(db, args.arguments)?;
        let schema = r#"
            CREATE TABLE x(
                "filename" TEXT,
                "value" TEXT
            );
        "#;

        let vtab = Self {
            base: unsafe { mem::zeroed() },
            db,
            path: arguments.dirname,
        };

        Ok((schema.to_string(), vtab))
    }

    fn destroy(&self) -> Result<()> {
        Ok(())
    }

    fn best_index(&self, mut info: IndexInfo) -> core::result::Result<(), BestIndexError> {
        // TODO: No matter how the set is queried, always jsut read from top to bottom,
        info.set_estimated_cost(10000.0);
        info.set_estimated_rows(10000);
        info.set_idxnum(1);

        Ok(())
    }

    fn open(&mut self) -> Result<Self::Cursor> {
        let records = read_data(&self.path).map_err(|err| Error::new_message(&err.to_string()))?;

        Self::Cursor::new(records)
    }
}

#[repr(C)]
pub struct TomlCursor {
    /// Base class. Must be first
    base: sqlite3_vtab_cursor,
    /// The record headers (i.e. the column names)
    headers: Headers,
    records: Records,
    /// Current cursor position used as rowid
    rowid: i64,
    eof: bool,
}

impl TomlCursor {
    fn new(records: Records) -> Result<Self> {
        let mut cursor = Self {
            base: unsafe { mem::zeroed() },
            headers: vec!["fieldname".to_string(), "value".to_string()],
            records,
            rowid: 0,
            eof: false,
        };

        cursor.next().map(|_| cursor)
    }
}

impl VTabCursor for TomlCursor {
    fn filter(
        &mut self,
        _idx_num: c_int,
        _idx_str: Option<&str>,
        _values: &[*mut sqlite3_value],
    ) -> Result<()> {
        Ok(())
    }

    fn next(&mut self) -> Result<()> {
        if self.rowid as usize == self.records.len() {
            self.eof = true;

            return Ok(());
        }

        self.rowid += 1;

        Ok(())
    }

    fn eof(&self) -> bool {
        self.eof
    }

    fn column(&self, ctx: *mut sqlite3_context, col_idx: c_int) -> Result<()> {
        if col_idx < 0 || col_idx as usize >= self.headers.len() {
            return Err(Error::new_message(&format!(
                "column index out of bounds: {col_idx}"
            )));
        }

        let row_idx = (self.rowid - 1) as usize;

        if let Some(record) = &self.records.get(row_idx) {
            api::result_text(ctx, &record[col_idx as usize].to_owned())?;
        } else {
            api::result_null(ctx);
        }

        Ok(())
    }

    fn rowid(&self) -> Result<i64> {
        Ok(self.rowid)
    }
}

#[derive(Debug, PartialEq)]
struct TomlArguments {
    dirname: String,
}

fn parse_arguments(db: *mut sqlite3, arguments: Vec<String>) -> Result<TomlArguments> {
    let mut dirname: Option<String> = None;

    for arg in arguments {
        match parse_argument(arg.as_str()) {
            Ok(arg) => match arg {
                Argument::Config(config) => match config.key.as_str() {
                    "dirname" => {
                        let value = parse_path(db, config.value)?;

                        dirname = Some(value);
                    }
                    _ => (),
                },
                Argument::Column(_column) => (),
            },
            Err(err) => return Err(Error::new_message(err.as_str())),
        };
    }

    let dirname = dirname.ok_or_else(|| Error::new_message("no dirname given. Specify a path to a directory containing TOML files to read from. E.g. 'dirname=\"my_toml_dir\"'"))?;

    Ok(TomlArguments { dirname })
}

pub fn parse_path(_db: *mut sqlite3, value: ConfigOptionValue) -> Result<String> {
    let value = match value {
        ConfigOptionValue::Quoted(value) => Ok(value),
        // ConfigOptionValue::SqliteParameter(value) => {
        //     match sqlite_parameter_value(db, value.as_str()) {
        //         Ok(result) => match result {
        //             Some(path) => Ok(path),
        //             None => Err(Error::new_message(
        //                 format!("{value} is not defined in temp.sqlite_parameters table").as_str(),
        //             )),
        //         },
        //         Err(_) => Err(Error::new_message(
        //             "temp.sqlite_parameters is not defined, can't use sqlite parameters as value",
        //         )),
        //     }
        // }
        _ => Err(Error::new_message(
            "'dirname' value must be a string. Wrap in single or double quotes.",
        )),
    }?;

    if !Path::new(&value).exists() {
        return Err(Error::new_message(
            &format!("dir '{value}' does not exist",),
        ));
    }

    Ok(value)
}
