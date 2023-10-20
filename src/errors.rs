use std::error;
use std::fmt;
use std::io;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    LineParse(String, usize),
    Io(io::Error),
    EnvVar(std::env::VarError),
}

impl Error {
    #[must_use]
    pub fn not_found(&self) -> bool {
        if let Self::Io(ref io_error) = *self {
            return io_error.kind() == io::ErrorKind::NotFound;
        }

        false
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::EnvVar(err) => Some(err),
            Self::LineParse(_, _) => None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(fmt, "{err}"),
            Self::EnvVar(err) => write!(fmt, "{err}"),
            Self::LineParse(line, error_index) => write!(
                fmt,
                "Error parsing line: '{line}', error at line index: {error_index}"
            ),
        }
    }
}

#[cfg(test)]
mod test {
    use std::error::Error as StdError;

    use super::*;

    #[test]
    fn test_io_error_source() {
        let err = Error::Io(std::io::ErrorKind::PermissionDenied.into());
        let io_err = err
            .source()
            .unwrap()
            .downcast_ref::<std::io::Error>()
            .unwrap();
        assert_eq!(std::io::ErrorKind::PermissionDenied, io_err.kind());
    }

    #[test]
    fn test_envvar_error_source() {
        let err = Error::EnvVar(std::env::VarError::NotPresent);
        let var_err = err
            .source()
            .unwrap()
            .downcast_ref::<std::env::VarError>()
            .unwrap();
        assert_eq!(&std::env::VarError::NotPresent, var_err);
    }

    #[test]
    fn test_lineparse_error_source() {
        let err = Error::LineParse("test line".to_string(), 2);
        assert!(err.source().is_none());
    }

    #[test]
    fn test_error_not_found_true() {
        let err = Error::Io(std::io::ErrorKind::NotFound.into());
        assert!(err.not_found());
    }

    #[test]
    fn test_error_not_found_false() {
        let err = Error::Io(std::io::ErrorKind::PermissionDenied.into());
        assert!(!err.not_found());
    }

    #[test]
    fn test_io_error_display() {
        let err = Error::Io(std::io::ErrorKind::PermissionDenied.into());
        let io_err: std::io::Error = std::io::ErrorKind::PermissionDenied.into();

        let err_desc = format!("{err}");
        let io_err_desc = format!("{io_err}");
        assert_eq!(io_err_desc, err_desc);
    }

    #[test]
    fn test_envvar_error_display() {
        let err = Error::EnvVar(std::env::VarError::NotPresent);
        let var_err = std::env::VarError::NotPresent;

        let err_desc = format!("{err}");
        let var_err_desc = format!("{var_err}");
        assert_eq!(var_err_desc, err_desc);
    }

    #[test]
    fn test_lineparse_error_display() {
        let err = Error::LineParse("test line".to_string(), 2);
        let err_desc = format!("{err}");
        assert_eq!(
            "Error parsing line: 'test line', error at line index: 2",
            err_desc
        );
    }
}
