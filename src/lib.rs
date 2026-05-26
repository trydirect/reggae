use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistrarError {
    NoRegistrarsConfigured,
    DomainUnavailable,
    ProviderFailure(String),
}

impl fmt::Display for RegistrarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoRegistrarsConfigured => write!(f, "no registrars configured"),
            Self::DomainUnavailable => write!(f, "domain is unavailable across all registrars"),
            Self::ProviderFailure(message) => write!(f, "registrar failure: {message}"),
        }
    }
}

impl Error for RegistrarError {}

pub trait DomainRegistrar {
    fn name(&self) -> &str;
    fn is_domain_available(&self, domain: &str) -> Result<bool, RegistrarError>;
    fn register_domain(&self, domain: &str, years: u8) -> Result<(), RegistrarError>;
}

pub struct UnifiedRegistrar {
    registrars: Vec<Box<dyn DomainRegistrar>>,
}

impl UnifiedRegistrar {
    pub fn new(registrars: Vec<Box<dyn DomainRegistrar>>) -> Self {
        Self { registrars }
    }

    pub fn add_registrar(&mut self, registrar: Box<dyn DomainRegistrar>) {
        self.registrars.push(registrar);
    }

    pub fn is_domain_available(&self, domain: &str) -> Result<bool, RegistrarError> {
        if self.registrars.is_empty() {
            return Err(RegistrarError::NoRegistrarsConfigured);
        }

        let mut saw_successful_check = false;

        for registrar in &self.registrars {
            match registrar.is_domain_available(domain) {
                Ok(true) => return Ok(true),
                Ok(false) => saw_successful_check = true,
                Err(_) => continue,
            }
        }

        if saw_successful_check {
            Ok(false)
        } else {
            Err(RegistrarError::ProviderFailure(
                "all registrars failed availability checks".to_string(),
            ))
        }
    }

    pub fn register_domain(&self, domain: &str, years: u8) -> Result<String, RegistrarError> {
        if self.registrars.is_empty() {
            return Err(RegistrarError::NoRegistrarsConfigured);
        }

        let mut saw_unavailable = false;
        let mut last_error = None;

        for registrar in &self.registrars {
            match registrar.is_domain_available(domain) {
                Ok(true) => match registrar.register_domain(domain, years) {
                    Ok(()) => return Ok(registrar.name().to_string()),
                    Err(error) => last_error = Some(error),
                },
                Ok(false) => saw_unavailable = true,
                Err(error) => last_error = Some(error),
            }
        }

        if let Some(error) = last_error {
            Err(error)
        } else if saw_unavailable {
            Err(RegistrarError::DomainUnavailable)
        } else {
            Err(RegistrarError::ProviderFailure(
                "all registrars failed".to_string(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct MockRegistrar {
        name: &'static str,
        available: Result<bool, RegistrarError>,
        register_result: Result<(), RegistrarError>,
        registrations: RefCell<Vec<(String, u8)>>,
    }

    impl MockRegistrar {
        fn new(
            name: &'static str,
            available: Result<bool, RegistrarError>,
            register_result: Result<(), RegistrarError>,
        ) -> Self {
            Self {
                name,
                available,
                register_result,
                registrations: RefCell::new(Vec::new()),
            }
        }
    }

    impl DomainRegistrar for MockRegistrar {
        fn name(&self) -> &str {
            self.name
        }

        fn is_domain_available(&self, _domain: &str) -> Result<bool, RegistrarError> {
            self.available.clone()
        }

        fn register_domain(&self, domain: &str, years: u8) -> Result<(), RegistrarError> {
            self.registrations
                .borrow_mut()
                .push((domain.to_string(), years));
            self.register_result.clone()
        }
    }

    #[test]
    fn availability_returns_true_if_any_provider_has_domain() {
        let registrar = UnifiedRegistrar::new(vec![
            Box::new(MockRegistrar::new("one", Ok(false), Ok(()))),
            Box::new(MockRegistrar::new("two", Ok(true), Ok(()))),
        ]);

        assert_eq!(registrar.is_domain_available("example.com"), Ok(true));
    }

    #[test]
    fn register_uses_first_provider_that_can_register() {
        let registrar = UnifiedRegistrar::new(vec![
            Box::new(MockRegistrar::new("one", Ok(false), Ok(()))),
            Box::new(MockRegistrar::new("two", Ok(true), Ok(()))),
        ]);

        assert_eq!(
            registrar.register_domain("example.com", 1),
            Ok("two".to_string())
        );
    }

    #[test]
    fn register_returns_unavailable_when_all_reject_domain() {
        let registrar = UnifiedRegistrar::new(vec![
            Box::new(MockRegistrar::new("one", Ok(false), Ok(()))),
            Box::new(MockRegistrar::new("two", Ok(false), Ok(()))),
        ]);

        assert_eq!(
            registrar.register_domain("example.com", 1),
            Err(RegistrarError::DomainUnavailable)
        );
    }

    #[test]
    fn availability_returns_failure_when_all_providers_error() {
        let registrar = UnifiedRegistrar::new(vec![
            Box::new(MockRegistrar::new(
                "one",
                Err(RegistrarError::ProviderFailure("a".to_string())),
                Ok(()),
            )),
            Box::new(MockRegistrar::new(
                "two",
                Err(RegistrarError::ProviderFailure("b".to_string())),
                Ok(()),
            )),
        ]);

        assert_eq!(
            registrar.is_domain_available("example.com"),
            Err(RegistrarError::ProviderFailure(
                "all registrars failed availability checks".to_string()
            ))
        );
    }

    #[test]
    fn register_falls_back_when_first_available_provider_fails_registration() {
        let registrar = UnifiedRegistrar::new(vec![
            Box::new(MockRegistrar::new(
                "one",
                Ok(true),
                Err(RegistrarError::ProviderFailure("unable to register".to_string())),
            )),
            Box::new(MockRegistrar::new("two", Ok(true), Ok(()))),
        ]);

        assert_eq!(
            registrar.register_domain("example.com", 1),
            Ok("two".to_string())
        );
    }

    #[test]
    fn empty_unified_registrar_returns_configuration_error() {
        let registrar = UnifiedRegistrar::new(Vec::new());

        assert_eq!(
            registrar.is_domain_available("example.com"),
            Err(RegistrarError::NoRegistrarsConfigured)
        );
        assert_eq!(
            registrar.register_domain("example.com", 1),
            Err(RegistrarError::NoRegistrarsConfigured)
        );
    }
}
