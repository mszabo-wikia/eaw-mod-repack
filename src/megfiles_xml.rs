use std::io::{BufReader, BufWriter, Read, Write};

use xml::{EmitterConfig, EventReader, reader::XmlEvent, writer};

static MEGA_FILES_ELEM: &str = "Mega_Files";

/// Write out a list of MEGA files into an XML document akin to megafiles.xml.
pub fn write_entries<W: Write>(writer: &mut W, entries: &Vec<String>) -> anyhow::Result<()> {
    let mut buf = BufWriter::new(writer);
    let mut writer = EmitterConfig::new()
        .perform_indent(true)
        .create_writer(&mut buf);

    writer.write(writer::XmlEvent::StartDocument {
        version: xml::common::XmlVersion::Version10,
        encoding: Some("utf8"),
        standalone: None,
    })?;

    writer.write(writer::XmlEvent::start_element(MEGA_FILES_ELEM))?;

    for file in entries {
        writer.write(writer::XmlEvent::start_element("File"))?;
        writer.write(writer::XmlEvent::Characters(file))?;
        writer.write(writer::XmlEvent::end_element())?;
    }

    writer.write(writer::XmlEvent::end_element())?;

    buf.write_all(b"\n")?;
    buf.flush()?;

    Ok(())
}

/// Read the list of MEGA files from an XML document akin to megafiles.xml.
pub fn get_entries<R: Read>(reader: R) -> anyhow::Result<Vec<String>> {
    let reader = BufReader::new(reader);
    let mut reader = EventReader::new(reader);

    let mut entries = vec![];

    loop {
        match reader.next()? {
            XmlEvent::StartElement { name, .. } if name.local_name == MEGA_FILES_ELEM => {
                handle_mega_files(&mut reader, &mut entries)?;
            }
            XmlEvent::EndDocument => break,
            _ => {}
        }
    }

    Ok(entries)
}

fn handle_mega_files<R>(
    reader: &mut EventReader<R>,
    entries: &mut Vec<String>,
) -> anyhow::Result<()>
where
    R: Read,
{
    loop {
        match reader.next()? {
            XmlEvent::StartElement { name, .. } if name.local_name == "File" => {
                handle_file(reader, entries)?;
            }
            XmlEvent::EndElement { name } if name.local_name == "Mega_Files" => {
                break;
            }
            _ => {}
        }
    }

    Ok(())
}

fn handle_file<R>(reader: &mut EventReader<R>, entries: &mut Vec<String>) -> anyhow::Result<()>
where
    R: Read,
{
    loop {
        match reader.next()? {
            XmlEvent::Characters(content) => entries.push(content),
            XmlEvent::EndElement { name } if name.local_name == "File" => {
                break;
            }

            _ => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        error::Error,
        io::{Cursor, Seek},
    };

    use crate::megfiles_xml;

    #[test]
    fn test_write_entries() -> Result<(), Box<dyn Error>> {
        let input_entries = vec!["Data\\Foo.meg".to_string(), "Data\\Bar.meg".to_string()];
        let mut buf = Cursor::new(Vec::<u8>::new());

        megfiles_xml::write_entries(&mut buf, &input_entries)?;

        assert_eq!(
            "<?xml version=\"1.0\" encoding=\"utf8\"?>\n<Mega_Files>\n  <File>Data\\Foo.meg</File>\n  <File>Data\\Bar.meg</File>\n</Mega_Files>\n",
            String::from_utf8(buf.into_inner())?
        );

        Ok(())
    }

    #[test]
    fn test_get_entries() -> Result<(), Box<dyn Error>> {
        let xml = "<?xml version=\"1.0\" encoding=\"utf8\"?>\n<Mega_Files>\n  <Info Name=\"MyMod\" Version=\"0.1.0\"/><File>Data\\Foo.meg</File>\n  <File>Data\\Bar.meg</File>\n</Mega_Files>\n";
        let buf = Cursor::new(xml);

        assert_eq!(
            vec!["Data\\Foo.meg", "Data\\Bar.meg"],
            megfiles_xml::get_entries(buf)?
        );

        Ok(())
    }

    #[test]
    fn test_roundtrip() -> Result<(), Box<dyn Error>> {
        let input_entries = vec!["Data\\Foo.meg".to_string(), "Data\\Bar.meg".to_string()];
        let mut buf = Cursor::new(Vec::<u8>::new());

        megfiles_xml::write_entries(&mut buf, &input_entries)?;

        buf.rewind()?;

        let output_entries = megfiles_xml::get_entries(buf)?;

        assert_eq!(input_entries, output_entries);

        Ok(())
    }
}
