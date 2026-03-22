pub struct FootNote {
    pub index: usize,
    pub name: String,
}

pub struct FootNotesData {
    footnotes: Vec<FootNote>,
}

const NUMBERING_NAME: &str = "!numbering";

fn numbering_name(index: usize) -> String {
    format!("footnote-{index}")
}
impl FootNotesData {
    pub fn new() -> Self {
        Self {
            footnotes: Vec::new(),
        }
    }

    pub fn add_footnote(&mut self, name: String) -> (String, usize) {
        let index = self.footnotes.len() + 1;
        let name = if name == NUMBERING_NAME {
            numbering_name(index)
        } else {
            name
        };
        self.footnotes.push(FootNote {
            index,
            name: name.clone(),
        });
        (name, index)
    }

    pub fn get_numbering(&self, name: &str) -> Option<usize> {
        for footnote in self.footnotes.iter() {
            if footnote.name == name {
                return Some(footnote.index);
            }
        }
        None
    }
}
