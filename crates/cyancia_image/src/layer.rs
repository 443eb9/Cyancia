use cyancia_id::Id;

#[derive(Debug)]
pub struct Layer {
    pub id: Id<Layer>,
}

impl Layer {
    pub fn new() -> Self {
        Self {
            id: Id::random(),
        }
    }

    pub fn id(&self) -> Id<Layer> {
        self.id
    }
}
