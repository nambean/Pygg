-- Who turned a model on or off: NULL is Pygg's default (on), 'user' the user's switch, and 'free-connection' Pygg,
-- because the model has a price on a connection set as free whose free part is a set of $0 models (OpenRouter's
-- :free, Z.AI's Flash). Pygg turns such a model back on when that stops being true; it never touches the user's.
ALTER TABLE models ADD COLUMN enabled_source TEXT;
