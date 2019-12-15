# smtpc

smtpc crate provides utils for parsing SMTP and multipart messages
Right now it's quite slow and requires some work to do on it.

It's NOT READY FOR PRODUCTION USE YET.

It was inspired by Go standard library which has lots of things already implemented
for Go programmer.

It tries do to all the decoding things in streaming manner so even
big messages can be easily processed.