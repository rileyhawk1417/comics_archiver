# Description

This tool/program, was built to help me archive my manga, comics. Basically [`.cbz` archives](https://en.wikipedia.org/wiki/Comic_book_archive) that I can throw into something like [Tachiyomi](https://tachiyomi.org/).
Most of the file contents are just scrapped images from websites. A `.cbz` archive makes it easier in the case of chapters. For example Chapter 1 will have about 90 images. Without that archive one would be having those images all over the place. 

Which wont help since it just makes it hard to find or organise them. Even if you did compress the whole comic folder (e.g "Horimiya") with all the chapters and everything in there. Its going to cost you space, since the images are huge in size.
So `.cbz` files are ideal if you are storing them.

### Why build this? 

Well I wanted to learn rust so thought why not 🤷. Although it might not be the best beginner friendly project.
Since this project involves compression, memory management, writing to disk and ensuring the data isnt corrupted.
I could have done it on `go`, `python`, `dart` or any other language really, but just wanted to write it in rust. Also accepting the pain that came along with it 😂.

For now its still in `alpha` since am literally trying everything to get it working. ChatGPT is helping when it comes to putting out rough code snippets. Since I havent really done any rust projects I cant say the code in here has best practices. Although I am going to refactor it as I go along, who knows might end up building a GUI after this...

### V2 Update

- Well in v2 I am to correct some mistakes in `v1` and hopefully improve the efficiency of the code.
- At some point might use [Tauri](https://tauri.app/) to build a GUI, though it would be a build it yourself case.


