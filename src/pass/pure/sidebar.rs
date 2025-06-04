use crate::ir::article::sidebar::{HeadingNumberingStyle, Pos, SidebarIndex, SidebarPos};
use crate::compile::registry::Key;
use crate::config::sidebar::SidebarConfig;
use crate::util::pos_slug;
use crate::util::str::SidebarElem;
use std::cmp::{Ordering, PartialEq};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub enum SectionData {
    Inner {
        level: usize,
        title: Vec<String>,
        children: Vec<Section>,
    },
    Embed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Section {
    pub anchor: String,
    pub data: SectionData,
}

impl Section {
    pub fn new(level: usize, anchor: String, title: Vec<String>) -> Self {
        Self {
            anchor,
            data: SectionData::Inner {
                level,
                title,
                children: Vec::new(),
            },
        }
    }
    pub fn new_embed(anchor: String) -> Self {
        Self {
            anchor,
            data: SectionData::Embed,
        }
    }
}

pub struct SidebarData<'a> {
    config: &'a SidebarConfig,
    pos: Pos,
    pub contents: Vec<String>,
    pub show_children: SidebarIndex,
    pub numberings: HashMap<Pos, SidebarIndex>,
    pub anchors: HashMap<Pos, SidebarIndex>,
    pub titles: HashMap<SidebarPos, SidebarIndex>,
}

impl<'a> SidebarData<'a> {
    fn push(&mut self, section: String) {
        self.contents.push(section);
    }
    fn push_heading_numbering(&mut self, pos: Pos) {
        self.numberings
            .entry(pos.clone())
            .or_default()
            .insert(self.contents.len());
        self.push(HeadingNumberingStyle::Bullet.display(&pos));
    }
    fn push_anchor(&mut self, pos: Pos,anchor:String) {
        self.anchors
            .entry(pos.clone())
            .or_default()
            .insert(self.contents.len());
        self.push(anchor);
    }
    fn push_show_children(&mut self) {
        self.show_children
            .insert(self.contents.len());
        self.push(String::new());
    }

    fn intake(&mut self, section: Section) {
        match section.data {
            SectionData::Inner {
                level: _level,
                title,
                children,
            } => {
                for elem in &self.config.each.body {
                    match elem {
                        SidebarElem::Plain(s) => self.push(s.to_string()),
                        SidebarElem::Anchor => self.push_anchor(self.pos.clone(), section.anchor.clone()),
                        SidebarElem::HeadingNumbering => self.push_heading_numbering(self.pos.clone()),
                        SidebarElem::ShowChildren => self.push_show_children(),
                        SidebarElem::Title => {
                            for (index, title) in title.clone().into_iter().enumerate() {
                                self.titles
                                    .entry((self.pos.clone(), index))
                                    .or_default()
                                    .insert(self.contents.len());
                                self.push(title);
                            }
                        }
                        SidebarElem::Children => {
                            self.pos.push(0);
                            for child in children.clone() {
                                self.intake(child);
                            }
                            self.pos.pop();
                        }
                    }
                }
            }
            SectionData::Embed => {
                let index = self.contents.len();
                self.titles
                    .entry((self.pos.clone(), 0))
                    .or_default()
                    .insert(index);
                self.push(String::new());
            }
        }
        self.pos
            .last_mut()
            .map(|u| *u += 1)
            .unwrap_or_else(|| self.pos.push(1));
    }

    fn build(config: &'a SidebarConfig, section: Section) -> SidebarData<'a> {
        let mut builder = Self {
            config,
            pos: Vec::new(),
            contents: Vec::new(),
            show_children: HashSet::new(),
            numberings: HashMap::new(),
            anchors: HashMap::new(),
            titles: HashMap::new(),
        };
        builder.intake(section);
        builder
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SidebarBuilder {
    slug: Key,
    pos: Pos,
    sections: Section,
}

impl SidebarBuilder {
    fn build(self, config: &SidebarConfig) -> SidebarData {
        SidebarData::build(config, self.sections)
    }

    fn new(slug: Key) -> Self {
        let anchor = slug.as_str()[1..].to_string();
        Self {
            slug,
            pos: vec![],
            sections: Section::new(0, anchor, vec!["".to_string()]),
        }
    }

    fn get_section(&self, pos: &[usize]) -> Option<&Section> {
        let mut option = Some(&self.sections);
        for &pos in pos {
            if let Some(section) = option {
                match &section.data {
                    SectionData::Inner { children, .. } => {
                        option = children.get(pos);
                    }
                    SectionData::Embed => return Some(section),
                }
            } else {
                break;
            }
        }
        option
    }

    fn get_section_mut<'a>(sections: &'a mut Section, pos: &[usize]) -> Option<&'a mut Section> {
        let mut current = sections;
        for &index in pos {
            current = match &current.data {
                SectionData::Inner { .. } => match &mut current.data {
                    SectionData::Inner { children, .. } => children.get_mut(index).unwrap(),
                    _ => panic!(),
                },
                SectionData::Embed => panic!(),
            };
        }
        Some(current)
    }

    fn current_level(&self) -> usize {
        let pos = self.pos.clone();
        self.get_level(&pos)
    }

    fn get_level(&self, pos: &[usize]) -> usize {
        self.get_section(pos)
            .map(|section| match section.data {
                SectionData::Inner { level, .. } => level,
                SectionData::Embed => 999,
            })
            .unwrap_or(0)
    }

    /*
     * Current level is recorded by the length of the current vector.
     * 1. If the level is greater than the current level, the current level is increased by 1.
     * 2. If the level is less than the current level, the current level is reduced to the level.
     */
    fn intake(&mut self, level: usize) -> Pos {
        if self.pos.is_empty() {
            self.pos.push(0);
            return self.pos.clone();
        }
        let mut pos = self.pos.clone();
        let current_level = self.current_level();
        let cmp = level.cmp(&current_level);
        match cmp {
            Ordering::Greater => {
                self.pos.push(0);
            }
            Ordering::Equal => *self.pos.last_mut().unwrap() += 1,
            Ordering::Less => {
                let mut last_pos: usize = *pos.last().unwrap();
                while !pos.is_empty() {
                    if self.get_level(&pos) < level {
                        pos.append(&mut vec![last_pos + 1]);
                        self.pos = pos.clone();
                        break;
                    }
                    last_pos = pos.pop().unwrap();
                }
                if pos.is_empty() {
                    self.pos = vec![last_pos + 1];
                }
            }
        }
        self.pos.clone()
    }

    fn add_section(&mut self, level: usize, title: Vec<String>) {
        let mut pos = self.pos.clone();
        let section = Section::new(level, pos_slug(&pos, self.slug.as_str()), title);
        pos.pop();
        let parent = Self::get_section_mut(&mut self.sections, &pos).unwrap();
        match &mut parent.data {
            SectionData::Inner { children, .. } => {
                children.push(section);
            }
            SectionData::Embed => panic!("Parent section is an embed section"),
        }
    }

    fn add_embed_section(&mut self, level: usize) -> Pos {
        self.intake(level);
        let parent_pos = {
            let mut pos = self.pos.clone();
            pos.pop();
            pos
        };
        let parent = Self::get_section_mut(&mut self.sections, &parent_pos).unwrap();
        match &mut parent.data {
            SectionData::Inner { children, .. } => {
                let anchor_str = pos_slug(&self.pos, self.slug.as_str());
                children.push(Section::new_embed(anchor_str));
            }

            SectionData::Embed => panic!("Parent section is an embed section"),
        }
        self.pos.clone()
    }
}

pub struct PureSidebarBuilder {
    full_sidebar: SidebarBuilder,
    only_embed_sidebar: SidebarBuilder,
}

impl PureSidebarBuilder {
    pub fn new(slug: Key) -> PureSidebarBuilder {
        Self {
            full_sidebar: SidebarBuilder::new(slug.clone()),
            only_embed_sidebar: SidebarBuilder::new(slug),
        }
    }

    pub fn intake_heading(&mut self, level: usize) -> Pos {
        self.full_sidebar.intake(level)
    }

    pub fn add_heading_section(&mut self, level: usize, title: Vec<String>) {
        self.full_sidebar.add_section(level, title);
    }
    pub fn current_full_pos(&self) -> Pos {
        self.full_sidebar.pos.clone()
    }

    pub fn add_embed_section(&mut self, level: usize) -> (Pos, Pos) {
        let full_sidebar_index = self.full_sidebar.add_embed_section(level);
        let only_embed_index = self.only_embed_sidebar.add_embed_section(level);
        (full_sidebar_index, only_embed_index)
    }

    pub fn build(self, config: &SidebarConfig) -> (SidebarData, SidebarData) {
        let full_sidebar = self.full_sidebar.build(config);
        let only_embed_sidebar = self.only_embed_sidebar.build(config);
        (full_sidebar, only_embed_sidebar)
    }
}
