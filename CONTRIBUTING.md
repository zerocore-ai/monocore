# Contributing to monocore

We welcome everyone to contribute what and where they can. Whether you are brand
new, just want to contribute a little bit, or want to contribute a lot there is
probably something you can help out with. Check out our
[good first issues][good-first-issues] label for in the issues tab to see a list
of issue that good for those new to the project.

## Where to Get Help

The main way to get help is by sending an email to [appcypher@outlook.com][support-email].
Though, this guide should help you get started. It may be slightly lengthy, but it's
designed for those who are new so please don't let length intimidate you.

## Code of Conduct

Please be kind, inclusive, and considerate when interacting when interacting
with others and follow our [code of conduct](./CODE_OF_CONDUCT.md).

## How to Contribute

If the code adds a feature that is not already present in an issue, you can
create a new issue for the feature and add the pull request to it. If the code
adds a feature that is not already present in an issue, you can create a new
issue for the feature and add the pull request to it.

### Contributing by Adding a Topic for Discussion

#### Issues

If you have found a bug and would like to report it or if you have a feature
that you feel we should add, then we'd love it if you opened an issue! ❤️
Before you do, please search the other issues to avoid creating a duplicate
issue.

To submit a new issue just hit the issue button and a choice between two
templates should appear. Then, follow along with the template you chose. If you
don't know how to fill in all parts of the template go ahead and skip those
parts. You can edit the issue later.

#### Discussion

If you have a new discussion you want to start but it isn't a bug or feature
add, then you can start a [GitHub discussion][gh-discussions]. Some examples of
what kinds of things that are good discussion topics can include, but are not
limited to the following:

-   Community announcements and/or asking the community for feedback
-   Discussing a new release
-   Asking questions, Q&A that isn't for sure a bug report

### Contributing through Code

In order to contribute through code follow the steps below. Note that you don't
need to be the best programmer to contribute.

 1. **Pick a feature** you would like to add or a bug you would like to fix
    - If you wish to contribute but what you want to fix/add is not already
      covered in an existing issue, please open a new issue.

 2. **Discuss** the issue with the rest of the community
    - Before you write any code, it is recommended that you discuss your
      intention to write the code on the issue you are attempting to edit.
    - This helps to stop you from wasting your time duplicating the work of
      others that maybe working on the same issue; at the same time.
    - This step also allows you to get helpful pointers on the community on some
      problems they may have encountered on similar issues.

 3. **Fork** the repository
    - A fork creates a copy of the code on your Github, so you can work on it
      separately from everyone else.
    - You can learn more about forking [here][forking].

 4. Ensure that you have **commit signing** enabled
    - This ensures that the code you submit was committed by you and not someone
      else who claims to be you.
    - You can learn more about how to setup commit signing [here][commit-signing].
	- If you have already made some commits that you wish to put in a pull
      request without signing them, then you can follow [this guide][post-signing]
      on how to fix that.

 5. **Clone** the repository to your local computer
    - This puts a copy of your fork on your computer so you can edit it
	- You can learn more about cloning repositories [here][git-clone].

 6. **Build** the project
    - For a detailed look on how to build monocore look at our
      [README file](./README.md).

 7. **Start writing** your code
    - Open up your favorite code editor and make the changes that you wanted to
      make to the repository.
    - Make sure to test your code with the test command(s) found in our
      [README file](./README.md).

 8. **Write tests** for your code
    - If you are adding a new feature, you should write tests that ensure that
      if someone make changes to the code it cannot break your new feature
      without breaking the test.
    - If your code adds a new feature, you should also write at least one
      documentation test. The documentation test's purpose is to demonstrate and
      document how to use the API feature.
    - If your code fixes a bug, you should write tests that ensure that if
      someone makes code changes in the future the bug does not re-emerge
      without breaking test.
    - Please create integration tests, if the addition is large enough to
      warrant them, and unit tests.
		  * Unit tests are tests that ensure the functionality of a single
      function or small section of code.
		  * Integration tests test large large sections of code.
		  * Read more about the differences [here][unit-and-integration].
    - For more information on test organization, take a look [here][test-org].

 9. Ensure that the code that you made follows our Rust **coding guidelines**
    - You can find a list of some Rust guidelines [here][rust-style-guide]. This
      is a courtesy to the programmers that come after you. The easier your code
      is to read, the easier it will be for the next person to make modifications.
    - If you find it difficult to follow the guidelines or if the guidelines or
      unclear, please reach out to us through our email linked above, or you
      can just continue and leave a comment at the pull request stage.

 10. **Commit and Push** your code
     - This sends your changes to your repository branch.
     - You can learn more about committing code [here][commiting-code] and
       pushing it to a remote repository [here][push-remote].
     - We use conventional commits for the names and description of commits.
       You can find out more about them [here][conventional-commits].

 11. The final step is to create **pull request** to our main branch 🎉
     - A pull request is how you merge the code you just worked so hard on with
       the code everyone else has access to.
	 - Once you have submitted your pull request, we will review your code and
       check to make sure the code implements the feature or fixes the bug. We
       may leave some feedback and suggest edits. You can make the changes we
       suggest by committing more code to your fork.
     - You can learn more about pull requests [here][prs].


[conventional-commits]: https://www.conventionalcommits.org/en/v1.0.0/
[commiting-code]: https://docs.github.com/en/desktop/contributing-and-collaborating-using-github-desktop/making-changes-in-a-branch/committing-and-reviewing-changes-to-your-project
[commit-signing]: https://www.freecodecamp.org/news/what-is-commit-signing-in-git/
[forking]: https://docs.github.com/en/get-started/quickstart/fork-a-repo
[gh-discussions]: https://docs.github.com/en/discussions
[git-clone]: https://docs.github.com/en/repositories/creating-and-managing-repositories/cloning-a-repository
[good-first-issues]: [https://build.prestashop-project.org/news/a-definition-of-the-good-first-issue-label/]
[post-signing]: https://dev.to/jmarhee/signing-existing-commits-with-gpg-5b58
[prs]: https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/about-pull-requests
[push-remote]: https://docs.github.com/en/get-started/using-git/pushing-commits-to-a-remote-repository
[rust-style-guide]: https://rust-lang.github.io/api-guidelines/about.html
[test-org]: https://doc.rust-lang.org/book/ch11-03-test-organization.html
[support-email]: mailto:appcypher@outlook.com
[unit-and-integration]: https://www.geeksforgeeks.org/difference-between-unit-testing-and-integration-testing/
