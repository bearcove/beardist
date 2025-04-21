pub(crate) struct IndentedWriter<'a> {
    inner: &'a mut String,
    indent_level: usize,
}

impl<'a> IndentedWriter<'a> {
    pub(crate) fn new(inner: &'a mut String) -> Self {
        IndentedWriter {
            inner,
            indent_level: 1,
        }
    }

    #[allow(dead_code)]
    pub(crate) fn indented(&mut self) -> IndentedWriter {
        IndentedWriter {
            inner: self.inner,
            indent_level: self.indent_level + 1,
        }
    }
}

impl std::fmt::Write for IndentedWriter<'_> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        // This function writes the input string to the inner buffer,
        // adding indentation at the start of each line.
        let mut needs_indent = self.inner.is_empty() || self.inner.ends_with('\n');

        for c in s.chars() {
            if needs_indent && c != '\n' {
                for _ in 0..self.indent_level {
                    self.inner.push_str("  ");
                }
                needs_indent = false;
            }
            self.inner.push(c);
            if c == '\n' {
                needs_indent = true;
            }
        }
        Ok(())
    }
}

pub(crate) trait Indented {
    fn indented(&mut self) -> IndentedWriter;
}

impl Indented for String {
    fn indented(&mut self) -> IndentedWriter {
        IndentedWriter::new(self)
    }
}
