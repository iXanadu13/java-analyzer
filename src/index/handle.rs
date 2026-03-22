use std::sync::Arc;

use arc_swap::ArcSwap;
use parking_lot::Mutex;

use crate::build_integration::WorkspaceModelSnapshot;
use crate::index::WorkspaceIndex;

#[derive(Clone)]
pub struct WorkspaceIndexHandle {
    current: Arc<ArcSwap<WorkspaceHandleState>>,
    write_serial: Arc<Mutex<()>>,
}

#[derive(Clone)]
struct WorkspaceHandleState {
    index: Arc<WorkspaceIndex>,
    model: Option<WorkspaceModelSnapshot>,
}

impl WorkspaceIndexHandle {
    pub fn new(index: WorkspaceIndex) -> Self {
        Self::new_with_model(index, None)
    }

    pub fn new_with_model(index: WorkspaceIndex, model: Option<WorkspaceModelSnapshot>) -> Self {
        Self {
            current: Arc::new(ArcSwap::from_pointee(WorkspaceHandleState {
                index: Arc::new(index),
                model,
            })),
            write_serial: Arc::new(Mutex::new(())),
        }
    }

    pub fn snapshot(&self) -> (Arc<WorkspaceIndex>, Option<WorkspaceModelSnapshot>) {
        let current = self.current.load_full();
        (Arc::clone(&current.index), current.model.clone())
    }

    pub fn load(&self) -> Arc<WorkspaceIndex> {
        self.snapshot().0
    }

    pub fn current_model(&self) -> Option<WorkspaceModelSnapshot> {
        self.snapshot().1
    }

    pub fn update<R>(&self, f: impl FnOnce(&WorkspaceIndex) -> R) -> R {
        let _guard = self.write_serial.lock();
        let current = self.current.load_full();
        f(current.index.as_ref())
    }

    pub fn replace(&self, index: WorkspaceIndex, model: Option<WorkspaceModelSnapshot>) {
        let _guard = self.write_serial.lock();
        self.current.store(Arc::new(WorkspaceHandleState {
            index: Arc::new(index),
            model,
        }));
    }

    pub fn replace_model(&self, model: Option<WorkspaceModelSnapshot>) {
        let _guard = self.write_serial.lock();
        let current = self.current.load_full();
        self.current.store(Arc::new(WorkspaceHandleState {
            index: Arc::clone(&current.index),
            model,
        }));
    }

    pub fn update_model<R>(&self, f: impl FnOnce(&mut Option<WorkspaceModelSnapshot>) -> R) -> R {
        let _guard = self.write_serial.lock();
        let current = self.current.load_full();
        let mut model = current.model.clone();
        let result = f(&mut model);
        self.current.store(Arc::new(WorkspaceHandleState {
            index: Arc::clone(&current.index),
            model,
        }));
        result
    }
}

impl Default for WorkspaceIndexHandle {
    fn default() -> Self {
        Self::new(WorkspaceIndex::new())
    }
}
