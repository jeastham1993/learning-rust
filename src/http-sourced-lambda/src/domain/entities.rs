use std::fmt;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use super::{
    error_types::{RepositoryError, ValidationError},
    public_types::{CreatedToDo, ToDoItem, ValidatedToDo, UnvalidatedToDo},
};

pub struct ToDoId {
    value: String,
}

impl ToDoId {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.len() > 0 {
            Ok(ToDoId { value })
        } else {
            Err(ValidationError::new(
                "To Id must be greater than 0".to_string(),
            ))
        }
    }

    pub fn get_value(&self) -> String {
        self.value.clone()
    }
}

pub struct Title {
    value: String,
}

impl Title {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.len() > 0 && value.len() <= 50 {
            Ok(Title { value })
        } else {
            Err(ValidationError::new(
                "Must be between 0 and 50 chars".to_string(),
            ))
        }
    }

    pub fn get_value(&self) -> String {
        self.value.clone()
    }
}

pub struct OwnerId {
    value: String,
}

impl OwnerId {
    pub fn new(value: String) -> Result<Self, ValidationError> {
        if value.len() > 0 {
            Ok(OwnerId { value })
        } else {
            Err(ValidationError::new("Must be greater than 0".to_string()))
        }
    }

    pub fn get_value(&self) -> String {
        self.value.clone()
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum IsComplete {
    INCOMPLETE,
    COMPLETE,
}

impl fmt::Display for IsComplete {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct ValidateToDo {
    title: Option<Title>,
    owner_id: Option<OwnerId>,
    is_complete: IsComplete,
    pub errors: Vec<ValidationError>,
    to_validate: UnvalidatedToDo
}

impl ValidateToDo {
    pub fn new(unvalidated_todo: UnvalidatedToDo) -> Self {
        ValidateToDo {
            title: Option::None,
            owner_id: Option::None,
            is_complete: IsComplete::INCOMPLETE,
            errors: Vec::new(),
            to_validate: unvalidated_todo
        }
    }

    pub fn validate(mut self) -> Result<ValidatedToDo, ValidationError> {
        self = self.check_title()
            .check_owner_id();
            
        if self.errors.len() > 0 {
            let mut errors = "".to_string();

            for ele in &self.errors {
                let message = format!("{} - {}", errors, ele.to_string()).to_string();

                errors = message.clone();
            }

            return Err(ValidationError::new(errors.to_string()));
        }

        Ok(ValidatedToDo {
            title: self.title.unwrap(),
            is_complete: self.is_complete,
            owner_id: self.owner_id.unwrap(),
        })
    }
    
    fn check_title(mut self) -> Self {
        let title = Title::new(self.to_validate.title.clone());

        match title {
            Ok(val) => self.title = Some(val),
            Err(e) => self.errors.push(e),
        };

        self
    }

    fn check_owner_id(mut self) -> Self {
        let owner_id = OwnerId::new(self.to_validate.owner_id.clone());

        match owner_id {
            Ok(val) => self.owner_id = Some(val),
            Err(e) => self.errors.push(e),
        };

        self
    }
}

#[async_trait]
pub trait Repository {
    async fn store_todo(&self, body: ValidatedToDo) -> Result<CreatedToDo, RepositoryError>;

    async fn get_todo(&self, id: &String) -> Result<ToDoItem, RepositoryError>;
}

/// Unit tests
///
/// These tests are run using the `cargo test` command.
#[cfg(test)]
mod tests {
    use crate::domain::public_types::UnvalidatedToDo;

    use super::ValidateToDo;

    #[test]
    fn valid_data_should_return_validated_to_do() {
        let validator = ValidateToDo::new(UnvalidatedToDo{
            is_complete: false,
            owner_id: "jameseastham".to_string(),
            title: "my title".to_string()
        });

        let to_do = validator.validate();

        let res = to_do.as_ref().unwrap();
        
        assert_eq!(to_do.is_err(), false);
        assert_eq!(res.title.get_value(), "my title");
        assert_eq!(res.owner_id.get_value(), "jameseastham");
        assert_eq!(res.is_complete.to_string(), "INCOMPLETE");
    }

    #[test]
    fn empty_title_should_return_validate_error() {
        let validator = ValidateToDo::new(UnvalidatedToDo{
            is_complete: false,
            owner_id: "jameseastham".to_string(),
            title: "".to_string()
        });

        let res = validator.validate();
        
        assert_eq!(res.is_err(), true);
        assert_eq!(res.err().unwrap().to_string(), "Validation error:  - Validation error: Must be between 0 and 50 chars");
    }

    #[test]
    fn empty_owner_should_return_validate_error() {
        let validator = ValidateToDo::new(UnvalidatedToDo{
            is_complete: false,
            owner_id: "".to_string(),
            title: "my title".to_string()
        });

        let res = validator.validate();
        
        assert_eq!(res.is_err(), true);
        assert_eq!(res.err().unwrap().to_string(), "Validation error:  - Validation error: Must be greater than 0");
    }
}