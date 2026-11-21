//! The dependencies between the core's modules equal the table in `table.rs`: a module uses
//! only the modules its row lists (or an exception names), every listed one is used, and the
//! rows hold no cycle. A new dependency fails here until the table says it is allowed.

mod blank;
mod blank_cases;
mod scan;
mod scan_cases;
mod table;
mod violations;
mod violations_cases;
