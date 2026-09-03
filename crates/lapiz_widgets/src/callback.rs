pub type Callback<'a, Message> = Option<Box<dyn FnOnce() -> Message + 'a>>;
pub type CallbackWith<'a, Input, Message> = Option<Box<dyn FnOnce(Input) -> Message + 'a>>;

pub fn publish<Message>(callback: &mut Callback<'_, Message>) -> Option<Message> {
    callback.take().map(|callback| callback())
}

pub fn publish_with<Input, Message>(
    callback: &mut CallbackWith<'_, Input, Message>,
    input: Input,
) -> Option<Message> {
    callback.take().map(|callback| callback(input))
}

#[macro_export]
macro_rules! callback_methods {
    ($field:ident) => {
        $crate::__private::paste! {
            pub fn [<on_ $field>](self, message: Message) -> Self
            where
                Message: 'a,
            {
                self.[<on_ $field _with_maybe>](Some(move || message))
            }

            pub fn [<on_ $field _maybe>](self, message: Option<Message>) -> Self
            where
                Message: 'a,
            {
                self.[<on_ $field _with_maybe>](message.map(|message| move || message))
            }

            pub fn [<on_ $field _with>](self, callback: impl FnOnce() -> Message + 'a) -> Self {
                self.[<on_ $field _with_maybe>](Some(callback))
            }

            pub fn [<on_ $field _with_maybe>]<F>(mut self, callback: Option<F>) -> Self
            where
                F: FnOnce() -> Message + 'a,
            {
                self.$field = callback.map(|callback| Box::new(callback) as _);
                self
            }
        }
    };
    ($field:ident, $input:ty) => {
        $crate::__private::paste! {
            pub fn [<on_ $field>](self, message: Message) -> Self
            where
                Message: 'a,
            {
                self.[<on_ $field _with_maybe>](Some(move |_: $input| message))
            }

            pub fn [<on_ $field _maybe>](self, message: Option<Message>) -> Self
            where
                Message: 'a,
            {
                self.[<on_ $field _with_maybe>](
                    message.map(|message| move |_: $input| message),
                )
            }

            pub fn [<on_ $field _with>](
                self,
                callback: impl FnOnce($input) -> Message + 'a,
            ) -> Self {
                self.[<on_ $field _with_maybe>](Some(callback))
            }

            pub fn [<on_ $field _with_maybe>]<F>(mut self, callback: Option<F>) -> Self
            where
                F: FnOnce($input) -> Message + 'a,
            {
                self.$field = callback.map(|callback| Box::new(callback) as _);
                self
            }
        }
    };
}
