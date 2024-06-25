for starting server print<br>
`cd ./tokio_test`<br>
`cargo run`<br>
for starting test client example create 5 different terminal tabs at dir<br>
`cd ./tokio_test_client`<br>
and write<br> 
`cargo run -- name_1 1`<br>
`cargo run -- name_2 1`<br>
`cargo run -- name_3 1`<br>
`cargo run -- name_4 qwe`<br>
`cargo run -- name_5 qwe`<br>
have time to write enough quickly, because the client has a message buffer that it can receive at once)<br>
each user may be stopped at any time using `ctrl+C` short
