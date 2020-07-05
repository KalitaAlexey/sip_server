// Request:
//   Find a dialog with call_id and from_tag
//     Didn't find:
//       Add a new dialog with call_id and from_tag
//   Has to_tag:
//   Doesn't have to_tag:
//     Create to_tag and respond with 100
//     Find a receiver
//       Found:
//         Add a new dialog with call_id, from_tag and to_tag
//         Create new call_id, from_tag, add a new dialog with them, link it to the previous dialog
//         Send the request to the receiver
//       Didn't find:
//         Respond with 404
//         Remove the dialog

#[derive(Debug)]
pub struct DialogInfo {
    call_id: String,
    server_tag: String,
    client_tag: String,
}

impl DialogInfo {
    pub fn new(call_id: String, server_tag: String, client_tag: String) -> Self {
        Self {
            call_id,
            server_tag,
            client_tag,
        }
    }
}

#[derive(Debug)]
pub struct IncompleteDialogInfo {
    call_id: String,
    server_tag: String,
}

impl IncompleteDialogInfo {
    pub fn new(call_id: String, server_tag: String) -> Self {
        Self {
            call_id,
            server_tag,
        }
    }
}

#[derive(Debug)]
pub struct Dialog {
    id: u32,
    call_id: String,
    server_tag: String,
    client_tag: String,
    linked_dialog: u32,
}

impl Dialog {
    pub fn call_id(&self) -> &String {
        &self.call_id
    }

    pub fn server_tag(&self) -> &String {
        &self.server_tag
    }

    pub fn client_tag(&self) -> &String {
        &self.client_tag
    }
}

#[derive(Debug)]
pub struct IncompleteDialog {
    id: u32,
    call_id: String,
    server_tag: String,
    linked_dialog: u32,
}

#[derive(Debug, Default)]
pub struct Dialogs {
    dialogs: Vec<Dialog>,
    incomplete_dialogs: Vec<IncompleteDialog>,
    next_dialog_id: u32,
}

impl Dialogs {
    pub fn new() -> Self {
        Dialogs::default()
    }

    pub fn add(&mut self, dialog_info: DialogInfo, incomplete_dialog_info: IncompleteDialogInfo) {
        let dialog_id = self.take_dialog_id();
        let incomplete_dialog_id = self.take_dialog_id();

        let dialog = Dialog {
            id: dialog_id,
            call_id: dialog_info.call_id,
            server_tag: dialog_info.server_tag,
            client_tag: dialog_info.client_tag,
            linked_dialog: incomplete_dialog_id,
        };
        self.dialogs.push(dialog);

        let incomplete_dialog = IncompleteDialog {
            id: incomplete_dialog_id,
            call_id: incomplete_dialog_info.call_id,
            server_tag: incomplete_dialog_info.server_tag,
            linked_dialog: dialog_id,
        };
        self.incomplete_dialogs.push(incomplete_dialog);
    }

    pub fn linked_dialog(
        &self,
        call_id: &str,
        server_tag: &str,
        client_tag: &str,
    ) -> Option<&Dialog> {
        let id = self.dialogs.iter().find_map(|d| {
            if d.call_id == call_id && d.server_tag == server_tag && d.client_tag == client_tag {
                Some(d.id)
            } else {
                None
            }
        });
        if let Some(id) = id {
            self.dialog_by_id(id)
        } else {
            None
        }
    }

    pub fn take_incomplete_dialog(
        &mut self,
        call_id: &str,
        server_tag: &str,
    ) -> Option<IncompleteDialog> {
        if let Some(index) = self
            .incomplete_dialogs
            .iter()
            .position(|d| d.call_id == call_id && d.server_tag == server_tag)
        {
            Some(self.incomplete_dialogs.swap_remove(index))
        } else {
            None
        }
    }

    pub fn complete_dialog(&mut self, dialog: IncompleteDialog, client_tag: String) {
        let dialog = Dialog {
            id: dialog.id,
            call_id: dialog.call_id,
            server_tag: dialog.server_tag,
            client_tag: client_tag,
            linked_dialog: dialog.linked_dialog,
        };
        self.dialogs.push(dialog);
    }

    fn dialog_by_id(&self, id: u32) -> Option<&Dialog> {
        self.dialogs.iter().find(|d| d.id == id)
    }

    fn take_dialog_id(&mut self) -> u32 {
        let id = self.next_dialog_id;
        self.next_dialog_id += 1;
        id
    }
}
