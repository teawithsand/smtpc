# SMTPC
[![Build Status](https://travis-ci.com/teawithsand/smtpc.svg?branch=master)](https://travis-ci.com/teawithsand/smtpc)

SMTPC crate provides utils for parsing messages sent over SMTP and multipart messages

It was heavily inspired(some parts are almost one-to-one rewritten from go) by Go standard library which has lots of things already implemented
and well supported for Go programmer.

SMTPC tries do to almost all decoding in streaming manner so even
big messages can be easily processed.

By default it supports serialization of some structures with serde