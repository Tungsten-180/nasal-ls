use lsp_server::{Message, Notification, Request, Response};
use lsp_types::{Location, Position, Range, Url};
use std::collections::{HashMap, LinkedList};

pub trait Verb {
    fn method(&self) -> &str;
    fn method_and(&self) -> (&str, Option<&serde_json::Value>);
}
impl Verb for Message {
    fn method(&self) -> &str {
        match self {
            Self::Request(req) => &req.method,
            Self::Notification(not) => &not.method,
            Self::Response(_resp) => &"response",
        }
    }
    fn method_and(&self) -> (&str, Option<&serde_json::Value>) {
        match self {
            Self::Request(req) => (&req.method, Some(&req.params)),
            Self::Notification(not) => (&not.method, Some(&not.params)),
            Self::Response(_resp) => (&"response", None),
        }
    }
}
#[derive(Clone, Debug)]
pub struct File {
    uri: String,
    text: String,
    ast: String,
    scopes: LinkedList<[u32; 2]>,
}
impl Default for File {
    #[inline]
    fn default() -> Self {
        File {
            uri: "".into(),
            text: "".into(),
            ast: "".into(),
            scopes: LinkedList::new(),
        }
    }
}
impl File {
    #[inline]
    pub fn with_text<S: Into<String>>(text: S) -> Self {
        File {
            uri: "".into(),
            text: text.into(),
            ast: "".into(),
            scopes: LinkedList::new(),
        }
    }
}
pub struct Library {
    catalog: std::collections::HashMap<String, File>,
}
impl Default for Library {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
impl Library {
    #[inline]
    pub fn new() -> Self {
        Self {
            catalog: Self::new_catalog(),
        }
    }
    #[inline]
    fn new_catalog() -> std::collections::HashMap<String, File> {
        std::collections::HashMap::new()
    }
    pub fn add_file(&mut self, filepath: String, text: String) -> Result<(), String> {
        let mut file = File::default();
        file.uri = filepath.clone();
        file.text = text.clone().into();
        file.ast = "".into();
        let res = Self::process_scopes(&mut file);
        self.catalog.insert(filepath.clone(), file);
        return res;
    }
    #[inline]
    fn get_file(&self, filepath: String) -> Option<File> {
        self.catalog.get(&filepath).cloned()
    }
    #[inline]
    fn get_file_or_blank(&self, filepath: String) -> File {
        let file: File = match self.catalog.get(&filepath) {
            Some(a) => a.clone(),
            None => File::default(),
        };
        file
    }
    fn process_scopes(file: &mut File) -> Result<(), String> {
        let mut failure = None;
        let mut line_number: u32 = 0;
        let mut scope_idx: usize = 0;
        //      [linenumber, scope_idx]
        let mut init_scopes: LinkedList<Initscope> = LinkedList::new();
        //       Option<[start, end]>
        let mut inter_scopes: LinkedList<Option<[u32; 2]>> = LinkedList::new();

        file.text.lines().for_each(|line| {
            if let Some(chars) = line.trim_start().get(..1) {
                if chars != "#" {
                    line.chars().for_each(|char| match char {
                        '{' => {
                            init_scopes.push_back(Initscope {
                                start: line_number,
                                idx: scope_idx,
                            });
                            inter_scopes.push_back(None);
                            scope_idx += 1;
                        }
                        '}' => match init_scopes.pop_back() {
                            Some(scope) => {
                                let mut back = inter_scopes.split_off(scope.idx);
                                back.pop_front();
                                back.push_front(Some([scope.start, line_number]));
                                inter_scopes.append(&mut back);
                            }
                            None => failure = Some(Err("Unmatched ".into())),
                        },
                        _ => {}
                    });
                }
                line_number += 1;
            }
        });
        let final_scopes: LinkedList<[u32; 2]> = inter_scopes
            .iter()
            .map(|opt: &Option<[u32; 2]>| match opt {
                Some(a) => a.clone(),
                None => {
                    failure = Some(Err("Scope Parse Failed".into()));
                    [0, 0]
                }
            })
            .collect();
        match failure {
            Some(err) => err,
            None => {
                file.scopes = final_scopes;
                Ok(())
            }
        }
    }
}
#[test]
fn scope_test() {
    let mut file = File::with_text(
        "
                               {
                                   {
                                       {
                                
                                       }
                                       #{
                                           {
                                           }
                                       
                                   }
                               }
                                   ",
    );
    assert_eq!(Library::process_scopes(&mut file).is_ok(), true);
}
#[derive(Debug)]
struct Initscope {
    start: u32,
    idx: usize,
}
pub struct Definitions {
    defs: std::collections::HashMap<String, LinkedList<NasalLspType>>,
}
#[derive(Clone)]
pub enum NasalLspType {
    FuncDef(Location),
    IdentDef(Location),
    Func(Location),
    IdentRef(Location),
}
impl NasalLspType {
    #[inline(always)]
    pub fn location(&self) -> &Location {
        match self {
            Self::Func(loc) => loc,
            Self::IdentRef(loc) => loc,
            Self::FuncDef(loc) => loc,
            Self::IdentDef(loc) => loc,
        }
    }
    #[inline]
    pub fn uri(&self) -> String {
        self.location().uri.as_str().into()
    }
}
trait Valid {
    fn is_valid(&self) -> bool;
    fn not_valid(&self) -> bool;
}
impl Valid for Location {
    #[inline(always)]
    fn is_valid(&self) -> bool {
        match (
            self.range.start.line < self.range.end.line,
            self.range.start.character < self.range.end.character,
        ) {
            (true, true) => true,
            (_, _) => false,
        }
    }
    #[inline(always)]
    fn not_valid(&self) -> bool {
        match (
            self.range.start.line < self.range.end.line,
            self.range.start.character < self.range.end.character,
        ) {
            (true, true) => false,
            (_, _) => true,
        }
    }
}
impl Definitions {
    #[inline]
    pub fn new() -> std::collections::HashMap<String, LinkedList<NasalLspType>> {
        std::collections::HashMap::new()
    }
    #[inline]
    fn new_list(&mut self, key: String) {
        if self.defs.insert(key, LinkedList::new()).is_some() {
            panic!()
        }
    }
    #[inline]
    pub fn add(&mut self, key: &String, def: NasalLspType) {
        match self.defs.get_mut(key) {
            Some(list) => {
                if list_search(&list, def.location()).is_none() {
                    list.push_front(def);
                }
            }
            None => self.new_list(key.clone()),
        }
    }
    #[inline]
    pub fn matches(&self, key: &String) -> Option<&LinkedList<NasalLspType>> {
        self.defs.get(key)
    }
    pub fn definition(&self, key: &String, loc: &Location) -> Result<NasalLspType, String> {
        if let Some(list) = self.matches(key) {
            match list.iter().fold(
                Err("No Values".to_string()),
                |res: Result<&NasalLspType, String>, node: &NasalLspType| {
                    let _ = 0;
                    let _ = match res {
                        Err(_) => res,
                        Ok(nasaltype) => res,
                    };
                    Ok(node)
                },
            ) {
                Ok(nasaltyperef) => Ok(nasaltyperef.clone()),
                Err(a) => Err(a),
            }
        } else {
            Err(format!("No definition exists for ident:{}", key))
        }
    }
}
#[inline]
fn list_search(list: &LinkedList<NasalLspType>, loc: &Location) -> Option<Location> {
    list.iter().fold(
        None,
        |answer: Option<Location>, node: &NasalLspType| match answer {
            Some(ans) => Some(ans),
            None => {
                if node.location() == loc {
                    Some(node.location().clone())
                } else {
                    None
                }
            }
        },
    )
}
