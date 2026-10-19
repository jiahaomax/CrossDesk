use std::collections::VecDeque;
#[derive(Debug, Clone, PartialEq)]
pub enum MotionItem<T> {
    Delta(i64, i64),
    Boundary(T),
}
/// Coalesce only adjacent motion. Backpressure must pause the session, never lose a key-up.
pub struct MotionQueue<T> {
    capacity: usize,
    items: VecDeque<MotionItem<T>>,
}
impl<T> MotionQueue<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            items: VecDeque::new(),
        }
    }
    pub fn push(&mut self, item: MotionItem<T>) -> Result<(), MotionItem<T>> {
        if let MotionItem::Delta(x, y) = &item {
            if let Some(MotionItem::Delta(a, b)) = self.items.back_mut() {
                if let (Some(nx), Some(ny)) = (a.checked_add(*x), b.checked_add(*y)) {
                    *a = nx;
                    *b = ny;
                    return Ok(());
                }
            }
        }
        if self.items.len() >= self.capacity {
            return Err(item);
        }
        self.items.push_back(item);
        Ok(())
    }
    pub fn pop(&mut self) -> Option<MotionItem<T>> {
        self.items.pop_front()
    }
}
