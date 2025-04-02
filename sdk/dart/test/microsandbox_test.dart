import 'package:microsandbox/microsandbox.dart';
import 'package:test/test.dart';

void main() {
  test('greet returns correct message', () {
    String result = greet('Test');
    expect(result, contains('Hello, Test!'));
  });
}
