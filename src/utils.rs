#[macro_export]
macro_rules! sql_enum {
	{ $vis:vis enum $name:ident { $($var:ident),* $(,)* } } => {
		#[derive(Debug, Clone, Copy, PartialEq, Eq)]
		$vis enum $name {
			$($var),*
		}

		impl AsRef<str> for $name {
			fn as_ref(&self) -> &str {
				match self {
					$(Self::$var => stringify!($var)),*
				}
			}
		}

		impl ::rusqlite::types::ToSql for $name {
			fn to_sql(&self) -> ::rusqlite::Result<::rusqlite::types::ToSqlOutput<'_>> {
				<str as ::rusqlite::types::ToSql>::to_sql(self.as_ref())
			}
		}
	}
}
