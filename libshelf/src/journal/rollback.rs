use std::io::Write;

use serde::Serialize;

use super::{Journal, JournalError, Record};

pub trait Rollback {
    type Output;

    fn rollback(&self) -> Self::Output;
}

/// An iterator that performs rollback on a [`Journal`]. See [`Journal::rollback`] and
/// [`Journal::rollback_last`].
#[derive(Debug)]
pub struct RollbackIter<'j, T, W>
where
    T: Rollback<Output = T> + Clone + Serialize,
    W: Write,
{
    journal: &'j mut Journal<T, W>,

    /// The current record index, where the oldest record has an index of 0.
    idx: usize,

    /// Flag that indicates whether or not any rollback records were appended.
    /// See [`RollbackIter::next`].
    appended: bool,
}

impl<T, W> Journal<T, W>
where
    T: Rollback<Output = T> + Clone + Serialize,
    W: Write,
{
    /// Return a [`RollbackIter`] that rolls-back until the last commit.
    ///
    /// If the latest record is a commit or there are no records, the iterator will do nothing.
    ///
    /// See [`RollbackIter`].
    #[inline]
    pub fn rollback(&mut self) -> RollbackIter<'_, T, W> {
        RollbackIter::new(self)
    }

    /// Return a [`RollbackIter`] that rolls-back the last transaction.
    ///
    /// If the latest record is a commit, the iterator will begin rolling back from the
    /// second-to-last commit; otherwise, this returns nothing.
    ///
    /// See [`RollbackIter`].
    #[inline]
    pub fn rollback_last(&mut self) -> Option<RollbackIter<'_, T, W>> {
        match self.latest()? {
            Record::Commit => Some(RollbackIter::new_idx(self, 1)),
            _ => None,
        }
    }
}

impl<'j, T, W> RollbackIter<'j, T, W>
where
    T: Rollback<Output = T> + Clone + Serialize,
    W: Write,
{
    /// Create a new rollback iterator at the latest reverse position.
    #[inline]
    fn new(journal: &'j mut Journal<T, W>) -> Self {
        Self::new_idx(journal, 0)
    }

    /// Create a new rollback iterator at the given reverse position.
    #[inline]
    fn new_idx(journal: &'j mut Journal<T, W>, idx: usize) -> Self {
        Self {
            journal,
            idx,
            appended: false,
        }
    }
}

impl<'j, T, W> Iterator for RollbackIter<'j, T, W>
where
    T: Rollback<Output = T> + Clone + Serialize,
    W: Write,
{
    type Item = Result<T, JournalError>;

    /// Look at the next record and perform the following operations depending on the record type:
    /// -   Action: append the record's rollback to the journal and return `Some` with the rollback
    ///             data.
    /// -   Commit or no record: if no rollback records have been appended yet, do nothing and
    ///     return `None`; otherwise, append a commit record to the journal and return `None`.
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (data, record) = match self.journal.get_back(self.idx) {
            Some(Record::Action(data)) => {
                let rdata = data.rollback();
                (Some(rdata.clone()), Record::Action(rdata))
            }
            Some(Record::Commit) | None => {
                if !self.appended {
                    return None;
                } else {
                    (None, Record::Commit)
                }
            }
        };

        self.idx += 1;

        // Append the record to the journal.
        match self.journal.append(record) {
            Ok(_) => {
                self.idx += 1;
                match data {
                    Some(data) => {
                        self.appended = true;
                        Some(Ok(data))
                    }
                    None => None,
                }
            }
            Err(err) => Some(Err(err)),
        }
    }
}

#[cfg(test)]
mod test {
    use super::super::test::{assert_writer, mk_expected, Datum, BACKWARD, COMMIT, FORWARD};
    use super::{Journal, Rollback};

    impl Rollback for Datum {
        type Output = Datum;

        #[inline]
        fn rollback(&self) -> Self::Output {
            match self {
                Datum::Forward => Self::Backward,
                Datum::Backward => Self::Forward,
            }
        }
    }

    #[test]
    fn test_rollback_empty() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal: Journal<Datum, _> = Journal::new(&mut writer);

        // No records; rollback does nothing.
        let mut rollback = journal.rollback();
        assert!(rollback.next().is_none());
        assert!(journal.is_empty());

        Ok(())
    }

    #[test]
    fn test_rollback_commit_only() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);

        journal.append(COMMIT)?;

        let mut rollback = journal.rollback();

        // Rollback should do nothing.
        assert!(rollback.next().is_none());
        assert_eq!(&[COMMIT], journal.records());

        Ok(())
    }

    #[test]
    fn test_rollback_commit_double() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);

        journal.append(COMMIT)?;
        journal.append(COMMIT)?;

        let mut rollback = journal.rollback();

        // Rollback should do nothing.
        assert!(rollback.next().is_none());
        assert_eq!(&[COMMIT, COMMIT], journal.records());

        Ok(())
    }

    #[test]
    fn test_rollback_no_commit() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);
        let mut records = Vec::new();

        records.push(FORWARD);
        journal.append(FORWARD)?;

        let mut rollback = journal.rollback();

        // Rollback should push a BACKWARD record to the journal on next.
        assert_eq!(Datum::Backward, rollback.next().unwrap()?);
        records.push(BACKWARD);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        // No more rollback to be done; None.
        assert!(rollback.next().is_none());
        // COMMIT record should have been pushed.
        records.push(COMMIT);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        // Same as last
        assert!(rollback.next().is_none());

        Ok(())
    }

    #[test]
    fn test_rollback_commit_last() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);

        journal.append(FORWARD)?;
        journal.append(COMMIT)?;

        let mut rollback = journal.rollback();

        // Rollback should do nothing.
        assert!(rollback.next().is_none());
        assert_eq!(&[FORWARD, COMMIT], journal.records());

        Ok(())
    }

    #[test]
    fn test_rollback_after_commit() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);
        let mut records = vec![FORWARD, COMMIT, FORWARD, BACKWARD];

        journal.append(FORWARD)?;
        journal.append(COMMIT)?;
        journal.append(FORWARD)?;
        journal.append(BACKWARD)?;

        let mut rollback = journal.rollback();

        // Rollback should push a FOWARD record to the journal on next.
        assert_eq!(Datum::Forward, rollback.next().unwrap()?);
        records.push(FORWARD);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        // Rollback should push a BACKWARD record to the journal on next.
        assert_eq!(Datum::Backward, rollback.next().unwrap()?);
        records.push(BACKWARD);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        // No more rollback to be done; None.
        assert!(rollback.next().is_none());
        // COMMIT record should have been pushed.
        records.push(COMMIT);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        Ok(())
    }

    #[test]
    fn test_rollback_last_empty() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal: Journal<Datum, _> = Journal::new(&mut writer);

        // No records; no rollback iter.
        let rollback = journal.rollback_last();
        assert!(rollback.is_none());

        Ok(())
    }

    #[test]
    fn test_rollback_last_non_commit() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);
        journal.append(FORWARD)?;

        // Latest is not commit; no rollback iter.
        let rollback = journal.rollback_last();
        assert!(rollback.is_none());

        Ok(())
    }

    #[test]
    fn test_rollback_last_normal() -> Result<(), Box<dyn std::error::Error>> {
        let mut writer = Vec::new();
        let mut journal = Journal::new(&mut writer);

        let mut records = vec![FORWARD, COMMIT];
        journal.append(FORWARD)?;
        journal.append(COMMIT)?;

        // Latest is commit; rollback iter.
        let mut rollback = journal.rollback_last().unwrap();

        // Rollback should push a BACKWARD record to the journal on next.
        assert_eq!(Datum::Backward, rollback.next().unwrap()?);
        records.push(BACKWARD);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        // End of transaction; rollback should return none.
        assert!(rollback.next().is_none());
        // COMMIT record should have been pushed.
        records.push(COMMIT);
        assert_writer(&mk_expected(&records)?, &rollback.journal);

        Ok(())
    }
}
