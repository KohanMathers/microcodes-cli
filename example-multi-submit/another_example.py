def check_for_fizzbuzz(n):
    if n % 3 == 0 and n % 5 == 0:
        return "FizzBuzz"
    elif n % 3 == 0:
        return "Fizz"
    elif n % 5 == 0:
        return "Buzz"
    else:
        return str(n)

def main():
    for i in range(1, 21):
        print(check_for_fizzbuzz(i))

if __name__ == "__main__":
    main()